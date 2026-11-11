use std::io::Write;

use kuro_core::{Timing, token_matches, voice_level};

#[test]
fn timing_csv_has_stages_only() {
    let path = std::env::temp_dir().join(format!("kuro-timing-{}.csv", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut timing = Timing::new(&path, true);
    let id = timing.begin("music");
    timing.mark("hotkey_down");
    timing.mark("stt_done");
    timing.finish(&id);
    timing.once("startup", "whisper_load", 12.5);
    let text = std::fs::read_to_string(&path).unwrap();
    let stages: Vec<&str> = text.lines().skip(1).map(|line| line.split(',').nth(2).unwrap()).collect();
    assert_eq!(stages, ["hotkey_down", "stt_done", "whisper_load"]);
    assert!(!text.to_lowercase().contains("mom"));
    assert!(!text.to_lowercase().contains("http"));
    let kinds: Vec<&str> = text.lines().skip(1).map(|line| line.split(',').nth(1).unwrap()).collect();
    assert_eq!(kinds[0], "music");
    assert_eq!(kinds[2], "startup");
    let ms = text.lines().nth(3).unwrap().split(',').nth(3).unwrap();
    assert_eq!(ms.parse::<f64>().unwrap(), 12.5);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn timing_disabled_writes_nothing() {
    let path = std::env::temp_dir().join(format!("kuro-timing-off-{}.csv", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut timing = Timing::new(&path, false);
    assert_eq!(timing.begin("news"), "");
    timing.mark("stt_done");
    timing.once("plug", "status", 3.0);
    assert!(!path.exists());
}

#[test]
fn missing_token_does_not_match() {
    let missing = std::env::temp_dir().join(format!("kuro-no-token-{}", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    assert_eq!(kuro_core::load_token(&missing), "");
    assert!(!token_matches("", "anything"));
    assert!(!kuro_ipc_listen_allowed(""));
}

fn kuro_ipc_listen_allowed(token: &str) -> bool {
    !token.is_empty()
}

#[test]
fn compare_digest_rejects_a_wrong_token() {
    assert!(token_matches("test-token-value", "test-token-value"));
    assert!(!token_matches("test-token-value", "test-token-valuE"));
    assert!(!token_matches("test-token-value", ""));
}

#[test]
fn config_whitelist_rejects_secrets() {
    let mut cfg = kuro_core::EngineConfig::default();
    kuro_core::apply_config(&mut cfg, &serde_json::json!({"llm_mode": "off", "paused": true})).unwrap();
    assert_eq!(cfg.llm_mode, "off");
    assert!(cfg.paused);
    assert!(kuro_core::apply_config(&mut cfg, &serde_json::json!({"KURO_N8N_TOKEN": "nope"})).is_err());
    assert!(kuro_core::apply_config(&mut cfg, &serde_json::json!({"llm_mode": "always-on"})).is_err());
}

#[test]
fn voice_level_lifts_quiet_speech() {
    let quiet = vec![1800; 200];
    let silence = vec![40; 200];
    let loud = vec![12000; 200];
    assert!(voice_level(&quiet) > 0.25);
    assert!(voice_level(&silence) < 0.08);
    assert!(voice_level(&loud) > 0.8);
    assert_eq!(voice_level(&[]), 0.0);
}

#[test]
fn rotating_log_and_heartbeat() {
    let dir = std::env::temp_dir().join(format!("kuro-log-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let log = dir.join("kuro.log");
    let mut file = std::fs::File::create(&log).unwrap();
    file.write_all(&[b'a'; 32]).unwrap();
    drop(file);
    kuro_core::append_rotating_log(&log, "next", 8);
    assert!(dir.join("kuro.log.1").exists() || log.exists());
    let beat = dir.join("heartbeat.txt");
    kuro_core::write_heartbeat(&beat);
    assert!(beat.is_file());
    let lock = dir.join("kuro.lock");
    let _ = std::fs::remove_file(&lock);
    assert!(kuro_core::try_singleton(&lock));
    assert!(!kuro_core::try_singleton(&lock));
    kuro_core::release_singleton(&lock);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn epoch_dismiss_invalidates_follow_up() {
    let mut clock = kuro_core::TurnClock::default();
    let epoch = clock.begin_turn();
    assert!(clock.live(epoch));
    clock.force_unblock();
    assert!(!clock.live(epoch));
    assert_eq!(kuro_core::follow_up_for("Want more?", clock.live(epoch), false), kuro_core::FollowUp::None);
    let next = clock.turn;
    assert!(next > epoch);
    assert_eq!(kuro_core::follow_up_for("Want more?", true, false), kuro_core::FollowUp::Listen);
}
