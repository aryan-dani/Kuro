use std::sync::{Arc, Mutex};

use kuro_core::{Timing, TurnClock};
use kuro_engine::voice_turn;

#[test]
fn voice_turn_records_stages_and_speaks() {
    let dir = std::env::temp_dir().join(format!("kuro-voice-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("timing.csv");
    let clock = Arc::new(Mutex::new(TurnClock::default()));
    let epoch = clock.lock().unwrap().begin_turn();
    let mut turn = voice_turn(Timing::new(&path, true));
    turn.hotkey_down();
    let reply = turn.run("cancel", "cancel", true, true, epoch, &clock, "off", false, |_| {});
    assert_eq!(reply, "Alright.");
    let text = std::fs::read_to_string(&path).unwrap();
    for stage in ["hotkey_down", "hotkey_up", "stt_done", "route_done", "tts_first_audio", "idle"] {
        assert!(text.contains(stage), "{stage} missing in {text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dismissed_turn_does_not_speak() {
    let dir = std::env::temp_dir().join(format!("kuro-voice-dismiss-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let clock = Arc::new(Mutex::new(TurnClock::default()));
    let epoch = clock.lock().unwrap().begin_turn();
    clock.lock().unwrap().force_unblock();
    let mut turn = voice_turn(Timing::new(dir.join("timing.csv"), false));
    turn.hotkey_down();
    let reply = turn.run("cancel", "cancel", true, true, epoch, &clock, "off", false, |_| {});
    assert!(reply.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
