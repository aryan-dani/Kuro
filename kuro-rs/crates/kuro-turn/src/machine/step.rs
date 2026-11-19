use super::effect::{Effect, HudMode};
use super::event::{Event, RouteOutcome};
use super::state::State;

/// Pure transition function (ADR 0008). No I/O, no clock reads.
pub fn step(state: State, event: Event) -> (State, Vec<Effect>) {
    match (state, event) {
        (State::Idle, Event::HotkeyDown) => {
            (State::Listening, vec![Effect::StartMic, Effect::SetHud(HudMode::Listening)])
        }

        (State::Listening, Event::HotkeyUp) => {
            (State::Transcribing, vec![Effect::StopMic, Effect::Transcribe, Effect::SetHud(HudMode::Thinking)])
        }
        (State::Listening, Event::Cancel) => cancel_from(State::Listening),

        (State::Transcribing, Event::SttDone { text }) => {
            if text.trim().is_empty() {
                (State::Idle, vec![Effect::SetHud(HudMode::Idle)])
            } else {
                (State::Routing, vec![Effect::Route])
            }
        }
        (State::Transcribing, Event::Error) => {
            (State::Idle, vec![Effect::SetHud(HudMode::Error), Effect::SetHud(HudMode::Idle)])
        }
        (State::Transcribing, Event::Cancel) => cancel_from(State::Transcribing),

        (State::Routing, Event::RouteDone(outcome)) => after_route(outcome),
        (State::Routing, Event::Cancel) => cancel_from(State::Routing),

        (State::Executing, Event::RouteDone(outcome)) => after_execute(outcome),
        (State::Executing, Event::Cancel) => cancel_from(State::Executing),

        (State::Confirming { plan_id, needs_execute, .. }, Event::ConfirmYes) => {
            let mut effects = vec![Effect::ExecuteConfirmed { plan_id }];
            if needs_execute {
                effects.push(Effect::Execute);
            }
            (State::Executing, effects)
        }
        (State::Confirming { .. }, Event::ConfirmNo | Event::ConfirmTimeout) => {
            (State::Idle, vec![Effect::SetHud(HudMode::Idle)])
        }

        (State::Speaking, Event::SpeakDone) => (State::Idle, vec![Effect::SetHud(HudMode::Idle)]),
        (State::Speaking, Event::HotkeyDown) => {
            (State::Listening, vec![Effect::CancelSpeak, Effect::StartMic, Effect::SetHud(HudMode::Listening)])
        }
        (State::Speaking, Event::Cancel) => cancel_from(State::Speaking),

        (State::FollowUp, Event::HotkeyUp) => {
            (State::Transcribing, vec![Effect::StopMic, Effect::Transcribe, Effect::SetHud(HudMode::Thinking)])
        }
        (State::FollowUp, Event::Cancel) => cancel_from(State::FollowUp),

        (State::Cancelling, Event::SpeakDone | Event::SttDone { .. } | Event::RouteDone(_)) => {
            (State::Idle, vec![Effect::SetHud(HudMode::Idle)])
        }
        (State::Cancelling, Event::Cancel | Event::Error) => (State::Idle, vec![Effect::SetHud(HudMode::Idle)]),

        (s, Event::Cancel) if s != State::Idle => cancel_from(s),

        (s @ State::Idle, _)
        | (s @ State::Listening, _)
        | (s @ State::Transcribing, _)
        | (s @ State::Routing, _)
        | (s @ State::Executing, _)
        | (s @ State::Confirming { .. }, _)
        | (s @ State::Speaking, _)
        | (s @ State::FollowUp, _)
        | (s @ State::Cancelling, _) => (s, Vec::new()),
    }
}

fn cancel_from(from: State) -> (State, Vec<Effect>) {
    let mut effects = Vec::new();
    if matches!(from, State::Listening | State::FollowUp) {
        effects.push(Effect::StopMic);
    }
    if matches!(from, State::Speaking) {
        effects.push(Effect::CancelSpeak);
    }
    effects.push(Effect::SetHud(HudMode::Idle));
    if matches!(from, State::Speaking) { (State::Cancelling, effects) } else { (State::Idle, effects) }
}

fn after_route(outcome: RouteOutcome) -> (State, Vec<Effect>) {
    if outcome.needs_confirm {
        return enter_confirming(outcome);
    }
    if outcome.needs_execute {
        return (State::Executing, vec![Effect::Execute]);
    }
    finish_with_reply(outcome)
}

fn after_execute(outcome: RouteOutcome) -> (State, Vec<Effect>) {
    if outcome.needs_confirm {
        return enter_confirming(outcome);
    }
    finish_with_reply(outcome)
}

fn enter_confirming(outcome: RouteOutcome) -> (State, Vec<Effect>) {
    let RouteOutcome { confirm_summary, confirm_tier, plan_id, needs_execute, reply, .. } = outcome;
    let summary = if confirm_summary.is_empty() { reply } else { confirm_summary };
    let state = State::Confirming { plan_id, tier: confirm_tier, summary: summary.clone(), needs_execute };
    let effects = vec![Effect::PromptConfirm { summary }, Effect::SetHud(HudMode::Thinking)];
    (state, effects)
}

fn finish_with_reply(outcome: RouteOutcome) -> (State, Vec<Effect>) {
    let RouteOutcome { reply, needs_execute: _, skip_tts, allow_follow_up, .. } = outcome;
    if reply.is_empty() {
        return (State::Idle, vec![Effect::SetHud(HudMode::Idle)]);
    }
    if skip_tts {
        if allow_follow_up {
            (State::FollowUp, vec![Effect::StartMic, Effect::SetHud(HudMode::Listening)])
        } else {
            (State::Idle, vec![Effect::SetHud(HudMode::Idle)])
        }
    } else {
        (State::Speaking, vec![Effect::Speak { text: reply.clone() }, Effect::SetHud(HudMode::Speaking)])
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::machine::Effect;

    #[test]
    fn speaking_hotkey_down_barge_in() {
        let (s, eff) = step(State::Speaking, Event::HotkeyDown);
        assert_eq!(s, State::Listening);
        assert!(eff.contains(&Effect::CancelSpeak));
        assert!(eff.contains(&Effect::StartMic));
        assert_eq!(eff.last(), Some(&Effect::SetHud(HudMode::Listening)));
    }

    #[test]
    fn idle_ignores_hotkey_up() {
        let (s, eff) = step(State::Idle, Event::HotkeyUp);
        assert_eq!(s, State::Idle);
        assert!(eff.is_empty());
    }

    #[test]
    fn speaking_speak_done_follow_up_when_skip_tts_and_allowed() {
        let (s, eff) = step(
            State::Routing,
            Event::RouteDone(RouteOutcome {
                reply: "Done.".into(),
                needs_execute: false,
                skip_tts: true,
                allow_follow_up: true,
                ..Default::default()
            }),
        );
        assert_eq!(s, State::FollowUp);
        assert!(eff.iter().any(|e| matches!(e, Effect::StartMic)));
    }
}
