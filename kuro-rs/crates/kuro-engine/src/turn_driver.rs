//! Drives `kuro_turn::step` for the post-hotkey-up slice of a voice turn.

use std::sync::{Arc, Mutex};

use kuro_core::{TurnClock, prefer_transcript};
use kuro_router::RouteResult;
use kuro_router::route_command;
use kuro_tools::brain::{Brain, chat};
use kuro_turn::{Event, RouteOutcome, State, TurnRunner};

use crate::turn_agent::{AgentHook, AgentRoute, try_agent_route};
use crate::turn_effects::apply_turn_effects;

pub(crate) struct RunContext<'a> {
    pub timing: &'a mut kuro_core::Timing,
    pub command_id: &'a str,
    pub speaker: &'a mut kuro_tts::Speaker,
    pub brain: &'a mut Brain,
    pub runner: &'a mut TurnRunner,
    pub agent: &'a AgentHook,
}

pub(crate) struct PostHotkeyInput<'a> {
    pub live: &'a str,
    pub final_text: &'a str,
    pub live_match: bool,
    pub final_match: bool,
    pub epoch: u64,
    pub clock: &'a Arc<Mutex<TurnClock>>,
    pub llm_mode: &'a str,
    pub ollama_up: bool,
}

pub(crate) fn run_post_hotkey_up(
    ctx: &mut RunContext<'_>,
    input: PostHotkeyInput<'_>,
    mut on_first_audio: impl FnMut(&str),
) -> String {
    let PostHotkeyInput { live, final_text, live_match, final_match, epoch, clock, llm_mode, ollama_up } = input;
    // Soft follow-up and re-entry often call run() from Idle (or leftover Speaking) without a
    // fresh HotkeyDown. Arm Listening first so HotkeyUp can reach Transcribing.
    arm_for_release(ctx.runner);
    let _release_effects = ctx.runner.on_event(Event::HotkeyUp);
    ctx.timing.mark("hotkey_up");

    let text = prefer_transcript(live, final_text, live_match, final_match);
    ctx.timing.mark("stt_done");
    let live_now = clock.lock().map(|c| c.live(epoch)).unwrap_or(false);

    if text.trim().is_empty() {
        let _ = ctx.runner.on_event(Event::SttDone { text });
        finish_idle(ctx);
        return String::new();
    }
    if !live_now {
        let _ = ctx.runner.on_event(Event::Cancel);
        finish_idle(ctx);
        return String::new();
    }

    let _ = ctx.runner.on_event(Event::SttDone { text: text.clone() });
    if ctx.runner.state() != State::Routing {
        // Never panic the voice thread (debug asserts used to kill the HUD after "thinking").
        crate::util::log_line(&format!(
            "turn stuck after SttDone: state={:?} text=`{}`",
            ctx.runner.state(),
            text.chars().take(80).collect::<String>()
        ));
        let _ = ctx.runner.on_event(Event::Cancel);
        finish_idle(ctx);
        return String::new();
    }

    let routed = route_command(&text, live_now, None, llm_mode, ollama_up);
    let (reply, skip_tts, outcome_override) = resolve_route(ctx, &text, &routed, epoch, llm_mode, ollama_up, live_now);
    ctx.timing.mark("route_done");
    let outcome = outcome_override.unwrap_or(RouteOutcome {
        reply: reply.clone(),
        needs_execute: false,
        skip_tts,
        allow_follow_up: false,
        ..Default::default()
    });
    let effects = ctx.runner.on_event(Event::RouteDone(outcome));
    apply_turn_effects(ctx, effects, epoch, clock, &mut on_first_audio);
    finish_idle(ctx);
    reply
}

fn resolve_route(
    ctx: &mut RunContext<'_>,
    text: &str,
    routed: &RouteResult,
    epoch: u64,
    llm_mode: &str,
    ollama_up: bool,
    live_now: bool,
) -> (String, bool, Option<RouteOutcome>) {
    if let Some(agent) = try_agent_route(ctx.agent, text, routed, llm_mode, epoch, epoch) {
        return match agent {
            AgentRoute::Reply(reply, skip_tts) => (reply, skip_tts, None),
            AgentRoute::Confirm(pending) => {
                let reply = pending.reply.clone();
                (reply, pending.skip_tts, Some(pending))
            }
        };
    }
    let (reply, skip_tts) = route_and_maybe_chat(ctx.brain, text, routed, live_now, ollama_up);
    (reply, skip_tts, None)
}

fn arm_for_release(runner: &mut TurnRunner) {
    match runner.state() {
        State::Listening | State::FollowUp => {}
        State::Idle | State::Speaking => {
            let _ = runner.on_event(Event::HotkeyDown);
        }
        State::Transcribing | State::Routing | State::Executing | State::Confirming { .. } | State::Cancelling => {
            let _ = runner.on_event(Event::Cancel);
            if runner.state() == State::Cancelling {
                let _ = runner.on_event(Event::SpeakDone);
            }
            if runner.state() == State::Idle {
                let _ = runner.on_event(Event::HotkeyDown);
            }
        }
    }
}

fn finish_idle(ctx: &mut RunContext<'_>) {
    ctx.timing.mark("idle");
    ctx.timing.finish(ctx.command_id);
}

fn route_and_maybe_chat(
    brain: &mut Brain,
    text: &str,
    routed: &RouteResult,
    live: bool,
    ollama_up: bool,
) -> (String, bool) {
    let mut routed = routed.clone();
    if routed.reply.is_empty() && routed.start_ollama && ollama_up && live {
        // Defense in depth: never ask the model to invent Meet/WhatsApp success.
        if kuro_tools::honesty::wants_tool_actuation(text) {
            routed.reply = kuro_tools::honesty::honest_tool_miss(text);
            return (routed.reply, false);
        }
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:e4b".into());
        let messages = brain.thread(text).to_vec();
        match chat(&model, &messages) {
            Ok(answer) => {
                let scrubbed = kuro_tools::honesty::scrub_chat_reply(text, &answer);
                routed.reply = brain.remember(&scrubbed);
            }
            Err(err) => routed.reply = err,
        }
    } else if routed.reply.is_empty() && routed.start_ollama && !ollama_up {
        routed.reply = "Neural engine is off. Please wait while I start it.".into();
    }
    (routed.reply, routed.skip_tts)
}

#[cfg(test)]
mod tests {
    use kuro_core::Timing;
    use kuro_llm::AgentConfig;
    use kuro_turn::{Effect, Event, HudMode, RouteOutcome, State, step};

    use super::*;
    use crate::turn_agent::AgentHook;
    use crate::voice::VoiceTurn;

    fn run_sequence(events: impl IntoIterator<Item = Event>) -> (State, Vec<Effect>) {
        let mut runner = TurnRunner::new();
        let mut all = Vec::new();
        for event in events {
            all.extend(runner.on_event(event));
        }
        (runner.state(), all)
    }

    #[test]
    fn post_release_happy_path_states_match_machine() {
        let (state, _) = run_sequence([
            Event::HotkeyDown,
            Event::HotkeyUp,
            Event::SttDone { text: "desk on".into() },
            Event::RouteDone(RouteOutcome {
                reply: "Desk on.".into(),
                needs_execute: false,
                skip_tts: false,
                allow_follow_up: false,
                ..Default::default()
            }),
            Event::SpeakDone,
        ]);
        assert_eq!(state, State::Idle);
    }

    #[test]
    fn voice_turn_run_tracks_listening_through_idle() {
        let dir = std::env::temp_dir().join(format!("kuro-turn-wire-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("timing.csv");
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let epoch = clock.lock().unwrap().begin_turn();
        let mut turn = VoiceTurn::new(Timing::new(&path, false));
        turn.hotkey_down(&clock);
        assert_eq!(turn.runner_state(), State::Listening);
        let reply = turn.run("thank you", "thank you", true, true, epoch, &clock, "off", false, |_| {});
        assert_eq!(reply, "Anytime.");
        assert_eq!(turn.runner_state(), State::Idle);
        let (s, _) = step(State::Listening, Event::HotkeyUp);
        assert_eq!(s, State::Transcribing);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn soft_follow_up_run_from_idle_does_not_panic() {
        let dir = std::env::temp_dir().join(format!("kuro-follow-idle-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("timing.csv");
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let epoch = clock.lock().unwrap().begin_turn();
        let mut turn = VoiceTurn::new(Timing::new(&path, false));
        turn.hotkey_down(&clock);
        let first = turn.run("thank you", "thank you", true, true, epoch, &clock, "off", false, |_| {});
        assert_eq!(first, "Anytime.");
        assert_eq!(turn.runner_state(), State::Idle);
        // Soft follow-up path calls run() again with no HotkeyDown (ptt_release.rs).
        let second = turn.run("what time is it", "what time is it", true, true, epoch, &clock, "off", false, |_| {});
        assert!(!second.is_empty(), "follow-up from Idle must still route");
        assert_eq!(turn.runner_state(), State::Idle);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_stt_reaches_idle_without_route() {
        let (state, effects) =
            run_sequence([Event::HotkeyDown, Event::HotkeyUp, Event::SttDone { text: String::new() }]);
        assert_eq!(state, State::Idle);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Route)));
        assert!(effects.contains(&Effect::SetHud(HudMode::Idle)));
    }

    #[test]
    fn agent_disabled_keeps_offline_reply() {
        let dir = std::env::temp_dir().join(format!("kuro-agent-off-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("timing.csv");
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let epoch = clock.lock().unwrap().begin_turn();
        let hook = AgentHook::from_config(AgentConfig::default());
        let mut turn = VoiceTurn::with_agent(Timing::new(&path, false), hook);
        turn.hotkey_down(&clock);
        let reply = turn.run("tell me a joke", "tell me a joke", true, true, epoch, &clock, "off", false, |_| {});
        assert!(reply.contains("don't have a phrase"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn barge_in_cancel_speak_on_hotkey_down() {
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let epoch = clock.lock().unwrap().begin_turn();
        let gen_before = clock.lock().unwrap().speak_gen;
        let dir = std::env::temp_dir().join(format!("kuro-barge-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut turn = VoiceTurn::new(Timing::new(dir.join("timing.csv"), false));
        let _ = turn.runner().on_event(kuro_turn::Event::HotkeyDown);
        let _ = turn.runner().on_event(kuro_turn::Event::HotkeyUp);
        let _ = turn.runner().on_event(kuro_turn::Event::SttDone { text: "hi".into() });
        let _ = turn.runner().on_event(kuro_turn::Event::RouteDone(RouteOutcome {
            reply: "Speaking.".into(),
            needs_execute: false,
            skip_tts: false,
            allow_follow_up: false,
            ..Default::default()
        }));
        assert_eq!(turn.runner_state(), State::Speaking);
        clock.lock().unwrap().begin_turn();
        turn.hotkey_down(&clock);
        assert!(clock.lock().unwrap().speak_gen > gen_before || !clock.lock().unwrap().speak_live(epoch, gen_before));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
