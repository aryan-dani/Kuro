#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use kuro_core::{Timing, TurnClock};
use kuro_engine::{
    AgentHook, PttArms, VoiceTurn, agent_config_from_settings, arm_ptt_hotkey, build_gate, build_wake_registry,
    engine_config_from_settings, exit_gate_ci, gate_ci_mode, install_panic_hook, load_and_apply_runtime_settings,
    load_engine_env, log_hotkey_arm, log_line, log_loaded_settings, log_wake_boot, set_hud, shutdown_hook,
    spawn_api_thread, spawn_heartbeat, spawn_initial_speech_warm, spawn_wifi_speech_watch,
};

fn main() {
    install_panic_hook();
    let gate_ci = gate_ci_mode();
    let repo = kuro_core::repo_root_hint();
    load_engine_env(&repo);
    let settings = load_and_apply_runtime_settings();
    log_wake_boot(&settings);
    let _wake_triggers = build_wake_registry(&settings);
    let paths = kuro_core::KuroPaths::from_env();
    let _ = std::fs::create_dir_all(&paths.root);
    // Allow Google Meet create/join only in this process; cargo test leaves it off.
    kuro_tools::meet::enable_live_meet();
    if !kuro_core::try_singleton(&paths.singleton_lock()) {
        eprintln!("Kuro engine is already running.");
        std::process::exit(0);
    }
    kuro_core::append_rotating_log(&paths.log(), "kuro-engine start", 1_000_000);
    log_loaded_settings(&paths.log());
    kuro_core::write_heartbeat(&paths.heartbeat());
    spawn_heartbeat(paths.heartbeat());
    let _singleton_mutex = kuro_core::hold_assistant_mutex();
    let policy = kuro_wifi::WifiPolicy::load(&paths.root);
    if policy.disabled {
        kuro_core::release_singleton(&paths.singleton_lock());
        std::process::exit(0);
    }
    let ssid = kuro_wifi::current_ssid();
    let at_home = policy.should_run(ssid.as_deref());
    let force_prewarm = matches!(std::env::var("KURO_STT_PREWARM").unwrap_or_default().as_str(), "1" | "true" | "on");
    spawn_initial_speech_warm(gate_ci, at_home, force_prewarm);
    spawn_wifi_speech_watch(gate_ci, paths.root.clone(), at_home, force_prewarm);
    let timing_on = matches!(
        std::env::var("KURO_TIMING").unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    );
    let shared_clock = Arc::new(Mutex::new(TurnClock::default()));
    let agent = AgentHook::from_config(agent_config_from_settings(&settings));
    let voice = Arc::new(Mutex::new(VoiceTurn::with_agent(Timing::new(paths.timing_csv(), timing_on), agent)));
    let (ptt, _) = kuro_audio::PttBuffer::new();
    let capture = Arc::new(Mutex::new(ptt));
    let capture_stream: Rc<RefCell<Option<kuro_audio::CaptureSession>>> = Rc::new(RefCell::new(None));
    let listening = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let busy = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let mut api_state: Option<Arc<kuro_ipc::AppState>> = None;
    if kuro_core::api_requested() {
        let token = kuro_core::ensure_token(&paths.api_token()).unwrap_or_default();
        if kuro_ipc::can_listen(&token) {
            let state = Arc::new(kuro_ipc::AppState {
                token,
                config: Mutex::new(engine_config_from_settings(&settings)),
                hud: Mutex::new(kuro_ipc::HudSnap::default()),
                contacts: Mutex::new(paths.contacts()),
                workflows: Mutex::new(paths.workflows()),
                timing: Mutex::new(Timing::new(paths.timing_csv(), timing_on)),
                brain: Mutex::new(kuro_tools::brain::Brain::default()),
                dismiss_hits: Mutex::new(0),
                shutdown_hits: Mutex::new(0),
                shutdown_hook: Mutex::new(Some(shutdown_hook())),
                clock: shared_clock.clone(),
                hud_wake: Arc::new(tokio::sync::Notify::new()),
                turn_busy: busy.clone(),
            });
            api_state = Some(state.clone());
            spawn_api_thread(state, settings.api_port, log_line);
        }
    }

    let gate = build_gate(busy.clone(), shared_clock.clone(), api_state.clone());
    exit_gate_ci(gate_ci, api_state.clone(), &paths.singleton_lock());

    if let Some(state) = api_state.as_ref() {
        set_hud(&state.hud, "idle", "idle", Some(vec![0.0; 11]), "", "", Some(&state.hud_wake));
    }

    let spec = kuro_hotkey::default_hotkey();
    log_hotkey_arm(&spec);
    arm_ptt_hotkey(
        PttArms { gate_ci, hud: api_state, capture, capture_stream, listening, gate, voice, clock: shared_clock },
        &paths.singleton_lock(),
    );
}
