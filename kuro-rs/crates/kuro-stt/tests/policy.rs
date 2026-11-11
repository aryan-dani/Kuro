use kuro_stt::{SttBackend, beam_size, choose_transcript, initial_prompt, ready_for_heavy, select_backend};
use serde_json::json;

#[test]
fn auto_picks_cuda_on_ac_and_npu_on_battery() {
    assert_eq!(select_backend("auto", true), SttBackend::Cuda);
    assert_eq!(select_backend("auto", false), SttBackend::Npu);
    assert_eq!(select_backend("cpu", true), SttBackend::Cpu);
}

#[test]
fn partial_beam_is_one() {
    assert_eq!(beam_size(true), 1);
}

#[test]
fn prompt_includes_aliases_and_contacts() {
    let text = initial_prompt(&["morning brief".into()], &["Siya".into()]);
    assert!(text.contains("Desk off"));
    assert!(text.contains("morning brief"));
    assert!(text.contains("Siya"));
}

#[test]
fn live_partial_wins_when_it_is_the_phrase() {
    let chosen = choose_transcript("desk off", "hello there", true, false);
    assert_eq!(chosen, "desk off");
    assert_eq!(choose_transcript("hello", "", false, false), "hello");
}

#[test]
fn legion_gate_needs_performance_mode() {
    let now = 1_000.0;
    let good = json!({"updated": 980.0, "ac_online": true, "smart_fan_mode": 3, "ac_fit": 1});
    assert!(ready_for_heavy(true, Some(&good), now));
    let weak = json!({"updated": 980.0, "ac_online": true, "smart_fan_mode": 3, "ac_fit": 0});
    assert!(!ready_for_heavy(true, Some(&weak), now));
    assert!(!ready_for_heavy(false, Some(&good), now));
}
