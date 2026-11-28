use std::path::Path;
use std::sync::Arc;

use kuro_ipc::AppState;

use crate::speech::{unload_speech, warm_ollama, warm_speech};
use crate::util::log_line;

pub const ENGINE_ENV_KEYS: &[&str] = &[
    "KURO_N8N_BASE_URL",
    "KURO_N8N_TOKEN",
    "OLLAMA_MODEL",
    "TUYA_DEVICE_ID",
    "TUYA_LOCAL_KEY",
    "TUYA_IP",
    "TUYA_VERSION",
    "KURO_NEONIZE_DLL",
    "KURO_TTS_ENGINE",
    "KURO_PIPER_MODEL",
    "KURO_PIPER_EXE",
    "KURO_KOKORO_VOICE",
    "KURO_KOKORO_SPEED",
];

pub fn load_engine_env(repo: &Path) {
    kuro_core::load_env_file(&repo.join(".env"), ENGINE_ENV_KEYS);
}

pub fn spawn_heartbeat(beat: std::path::PathBuf) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(15));
            kuro_core::write_heartbeat(&beat);
        }
    });
}

pub fn spawn_initial_speech_warm(gate_ci: bool, at_home: bool, force_prewarm: bool) {
    if gate_ci {
        return;
    }
    if at_home || force_prewarm {
        std::thread::spawn(warm_speech);
    } else {
        std::thread::spawn(warm_ollama);
    }
}

pub fn spawn_wifi_speech_watch(gate_ci: bool, watch_root: std::path::PathBuf, at_home: bool, force_prewarm: bool) {
    if gate_ci {
        return;
    }
    std::thread::spawn(move || {
        let mut was_home = at_home || force_prewarm;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if force_prewarm {
                continue;
            }
            let policy = kuro_wifi::WifiPolicy::load(&watch_root);
            let connected = kuro_wifi::current_ssid();
            let home_now = policy.should_run(connected.as_deref());
            if !home_now {
                unload_speech();
                was_home = false;
            } else if !was_home {
                warm_speech();
                was_home = true;
            }
        }
    });
}

pub fn exit_gate_ci(gate_ci: bool, api_state: Option<Arc<AppState>>, singleton_lock: &Path) {
    if !gate_ci {
        return;
    }
    let Some(state) = api_state else {
        eprintln!("KURO_GATE_CI requires KURO_API=on with a valid api_token.");
        kuro_core::release_singleton(singleton_lock);
        std::process::exit(1);
    };
    for _ in 0..80 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if *state.shutdown_hits.lock().unwrap_or_else(|e| e.into_inner()) > 0 {
            kuro_core::release_singleton(singleton_lock);
            std::process::exit(0);
        }
    }
    kuro_core::release_singleton(singleton_lock);
    std::process::exit(1);
}

pub fn shutdown_hook() -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(unload_speech)
}

pub fn log_hotkey_arm(spec: &kuro_hotkey::HotkeySpec) {
    log_line(&format!(
        "hotkey arming ctrl={} alt={} shift={} win={} vk=0x{:02X} (LL hook)",
        spec.ctrl, spec.alt, spec.shift, spec.win, spec.vk
    ));
}
