//! Scenario matrix for the pure turn machine (phase 12 slice).

use kuro_turn::{Effect, Event, HudMode, RouteOutcome, State, step};

fn run_sequence(events: impl IntoIterator<Item = Event>) -> (State, Vec<Effect>) {
    let mut state = State::Idle;
    let mut all = Vec::new();
    for event in events {
        let (next, effects) = step(state, event);
        state = next;
        all.extend(effects);
    }
    (state, all)
}

#[test]
fn happy_path_hold_to_talk() {
    let (state, effects) = run_sequence([
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
    let kinds: Vec<&str> = effects
        .iter()
        .map(|e| match e {
            Effect::StartMic => "start_mic",
            Effect::StopMic => "stop_mic",
            Effect::Transcribe => "transcribe",
            Effect::Route => "route",
            Effect::Execute => "execute",
            Effect::Speak { .. } => "speak",
            Effect::CancelSpeak => "cancel_speak",
            Effect::PromptConfirm { .. } => "prompt_confirm",
            Effect::ExecuteConfirmed { .. } => "execute_confirmed",
            Effect::SetHud(_) => "set_hud",
        })
        .collect();
    assert_eq!(
        kinds,
        ["start_mic", "set_hud", "stop_mic", "transcribe", "set_hud", "route", "speak", "set_hud", "set_hud",]
    );
    assert!(effects.iter().any(|e| matches!(e, Effect::Speak { text } if text == "Desk on.")));
}

#[test]
fn cancel_from_listening() {
    let (state, effects) = run_sequence([Event::HotkeyDown, Event::Cancel]);
    assert_eq!(state, State::Idle);
    assert!(effects.contains(&Effect::StopMic));
    assert!(effects.contains(&Effect::SetHud(HudMode::Idle)));
}

#[test]
fn cancel_from_speaking() {
    let (state, after_cancel) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "help".into() },
        Event::RouteDone(RouteOutcome {
            reply: "Desk".into(),
            needs_execute: false,
            skip_tts: false,
            allow_follow_up: false,
            ..Default::default()
        }),
        Event::Cancel,
    ]);
    assert_eq!(state, State::Cancelling);
    assert!(after_cancel.iter().any(|e| matches!(e, Effect::Speak { .. })));
    assert!(after_cancel.contains(&Effect::CancelSpeak));

    let (state, _) = step(state, Event::SpeakDone);
    assert_eq!(state, State::Idle);
}

fn route_to_speaking(reply: &str) -> impl IntoIterator<Item = Event> {
    [
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "help".into() },
        Event::RouteDone(RouteOutcome {
            reply: reply.into(),
            needs_execute: false,
            skip_tts: false,
            allow_follow_up: false,
            ..Default::default()
        }),
    ]
}

#[test]
fn barge_in_hotkey_down_from_speaking_enters_listening() {
    let (state, effects) = run_sequence(route_to_speaking("Desk").into_iter().chain([Event::HotkeyDown]));
    assert_eq!(state, State::Listening);
    assert!(effects.contains(&Effect::CancelSpeak));
    assert!(effects.contains(&Effect::StartMic));
    assert!(effects.contains(&Effect::SetHud(HudMode::Listening)));
}

#[test]
fn barge_in_then_release_transcribes() {
    let (state, effects) = run_sequence(route_to_speaking("Desk").into_iter().chain([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "desk off".into() },
    ]));
    assert_eq!(state, State::Routing);
    assert!(effects.contains(&Effect::CancelSpeak));
    assert!(effects.contains(&Effect::Transcribe));
}

#[test]
fn stt_fail_returns_idle() {
    let (state, effects) = run_sequence([Event::HotkeyDown, Event::HotkeyUp, Event::Error]);
    assert_eq!(state, State::Idle);
    assert!(effects.contains(&Effect::SetHud(HudMode::Error)));
    assert!(effects.contains(&Effect::SetHud(HudMode::Idle)));
}

#[test]
fn empty_transcript_skips_route() {
    let (state, effects) = run_sequence([Event::HotkeyDown, Event::HotkeyUp, Event::SttDone { text: String::new() }]);
    assert_eq!(state, State::Idle);
    assert!(!effects.iter().any(|e| matches!(e, Effect::Route)));
}

#[test]
fn execute_path_before_speak() {
    let (state, effects) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "stop the music".into() },
        Event::RouteDone(RouteOutcome {
            reply: String::new(),
            needs_execute: true,
            skip_tts: false,
            allow_follow_up: false,
            ..Default::default()
        }),
        Event::RouteDone(RouteOutcome {
            reply: "Music stopped.".into(),
            needs_execute: false,
            skip_tts: false,
            allow_follow_up: false,
            ..Default::default()
        }),
        Event::SpeakDone,
    ]);
    assert_eq!(state, State::Idle);
    assert!(effects.contains(&Effect::Execute));
}

fn confirm_route_outcome() -> RouteOutcome {
    RouteOutcome {
        reply: "Send WhatsApp to Sam?".into(),
        needs_execute: true,
        skip_tts: false,
        allow_follow_up: false,
        needs_confirm: true,
        confirm_summary: "Send WhatsApp to Sam.".into(),
        confirm_tier: "external_send".into(),
        plan_id: 42,
    }
}

fn reach_confirming() -> State {
    let (state, _) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "send whatsapp".into() },
        Event::RouteDone(confirm_route_outcome()),
    ]);
    state
}

#[test]
fn confirm_yes_executes() {
    let confirming = reach_confirming();
    assert!(matches!(confirming, State::Confirming { plan_id: 42, needs_execute: true, .. }));
    let (state, effects) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "send whatsapp".into() },
        Event::RouteDone(confirm_route_outcome()),
        Event::ConfirmYes,
        Event::RouteDone(RouteOutcome {
            reply: "Sent.".into(),
            needs_execute: false,
            skip_tts: false,
            allow_follow_up: false,
            ..Default::default()
        }),
        Event::SpeakDone,
    ]);
    assert_eq!(state, State::Idle);
    assert!(effects.iter().any(|e| matches!(e, Effect::PromptConfirm { .. })));
    assert!(effects.contains(&Effect::ExecuteConfirmed { plan_id: 42 }));
    assert!(effects.contains(&Effect::Execute));
}

#[test]
fn confirm_no_cancels() {
    let (state, effects) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "send whatsapp".into() },
        Event::RouteDone(confirm_route_outcome()),
        Event::ConfirmNo,
    ]);
    assert_eq!(state, State::Idle);
    assert!(effects.contains(&Effect::SetHud(HudMode::Idle)));
    assert!(!effects.contains(&Effect::Execute));
}

#[test]
fn confirm_timeout_is_no() {
    let (state, effects) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "send whatsapp".into() },
        Event::RouteDone(confirm_route_outcome()),
        Event::ConfirmTimeout,
    ]);
    assert_eq!(state, State::Idle);
    assert!(effects.contains(&Effect::SetHud(HudMode::Idle)));
    assert!(!effects.contains(&Effect::Execute));
}

#[test]
fn cancel_from_confirming() {
    let (state, effects) = run_sequence([
        Event::HotkeyDown,
        Event::HotkeyUp,
        Event::SttDone { text: "send whatsapp".into() },
        Event::RouteDone(confirm_route_outcome()),
        Event::Cancel,
    ]);
    assert_eq!(state, State::Idle);
    assert!(effects.contains(&Effect::SetHud(HudMode::Idle)));
}

fn cancel_reaches_idle(from: State) {
    let label = format!("{from:?}");
    let (mut state, _) = step(from, Event::Cancel);
    if state == State::Cancelling {
        let (next, _) = step(state, Event::SpeakDone);
        state = next;
    }
    assert_eq!(state, State::Idle, "cancel from {label}");
}

#[test]
fn cancel_from_every_non_idle_state_reaches_idle() {
    let states = [
        State::Listening,
        State::Transcribing,
        State::Routing,
        State::Executing,
        State::Speaking,
        State::FollowUp,
        State::Cancelling,
        State::Confirming {
            plan_id: 1,
            tier: "irreversible".into(),
            summary: "Delete files.".into(),
            needs_execute: true,
        },
    ];
    for from in states {
        cancel_reaches_idle(from);
    }
}
