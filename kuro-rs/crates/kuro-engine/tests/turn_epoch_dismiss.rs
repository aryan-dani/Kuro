use std::sync::{Arc, Mutex};

use kuro_core::{FollowUp, Timing, TurnClock, follow_up_for};
use kuro_engine::voice_turn;

#[test]
fn dismiss_increments_epoch_and_blocks_stale_follow_up() {
    let dir = std::env::temp_dir().join(format!("kuro-epoch-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let clock = Arc::new(Mutex::new(TurnClock::default()));
    let epoch = clock.lock().unwrap().begin_turn();
    assert!(clock.lock().unwrap().live(epoch));
    clock.lock().unwrap().force_unblock();
    assert!(!clock.lock().unwrap().live(epoch));
    assert_eq!(follow_up_for("Want more?", clock.lock().unwrap().live(epoch), false), FollowUp::None);
    assert!(clock.lock().unwrap().turn > epoch);
    let fresh = clock.lock().unwrap().begin_turn();
    assert!(clock.lock().unwrap().live(fresh));
    let mut turn = voice_turn(Timing::new(dir.join("timing.csv"), false));
    turn.hotkey_down();
    let spoken = turn.run("who are you", "who are you", true, true, fresh, &clock, "off", false, |_| {});
    assert_eq!(spoken, "Kuro. Local assistant. Phrases first, model last.");
    let _ = std::fs::remove_dir_all(&dir);
}
