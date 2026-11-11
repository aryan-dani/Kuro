use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use kuro_core::{Timing, TurnClock};
use kuro_engine::VoiceTurn;

fn unload_speech() {
    kuro_stt::shutdown_worker();
    kuro_tts::shutdown_worker();
}

fn warm_speech() {
    let _ = kuro_stt::prewarm();
    kuro_tts::prebuild_phrase_cache();
}

fn gate_ci_mode() -> bool {
    matches!(
        std::env::var("KURO_GATE_CI").unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn set_hud(hud: &Mutex<kuro_ipc::HudSnap>, state: &str, label: &str, levels: Option<Vec<f64>>, you: &str, kuro: &str) {
    let Ok(mut snap) = hud.lock() else {
        return;
    };
    snap.state = state.into();
    snap.label = if label.is_empty() { state.into() } else { label.into() };
    let cleared = levels.is_none();
    if let Some(levels) = levels {
        snap.levels = levels;
    }
    if !you.is_empty() {
        snap.you = you.into();
    }
    if !kuro.is_empty() {
        snap.kuro = kuro.into();
    }
    if state == "hidden" || state == "idle" {
        if state == "hidden" {
            snap.you.clear();
            snap.kuro.clear();
        }
        if cleared {
            snap.levels = vec![0.0; 11];
        }
    }
}

fn log_line(msg: &str) {
    let paths = kuro_core::KuroPaths::from_env();
    kuro_core::append_rotating_log(&paths.log(), msg, 1_000_000);
}

fn ollama_up() -> bool {
    let Ok(addr) = "127.0.0.1:11434".parse() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok()
}

fn main() {
    let gate_ci = gate_ci_mode();
    let repo = kuro_core::repo_root_hint();
    kuro_core::load_env_file(
        &repo.join(".env"),
        &[
            "KURO_N8N_BASE_URL",
            "KURO_N8N_TOKEN",
            "OLLAMA_MODEL",
            "KURO_PYTHON",
            "TUYA_DEVICE_ID",
            "TUYA_LOCAL_KEY",
            "TUYA_IP",
            "TUYA_VERSION",
            "KURO_TUYA_PYTHON_FALLBACK",
            "KURO_NEONIZE_DLL",
        ],
    );
    if std::env::var_os("KURO_N8N_BASE_URL").is_none_or(|v| v.is_empty()) {
        unsafe {
            std::env::set_var("KURO_N8N_BASE_URL", "http://100.78.60.50:5678/webhook");
        }
    }
    let paths = kuro_core::KuroPaths::from_env();
    let _ = std::fs::create_dir_all(&paths.root);
    kuro_core::append_rotating_log(&paths.log(), "kuro-engine start", 1_000_000);
    kuro_core::write_heartbeat(&paths.heartbeat());
    if !kuro_core::try_singleton(&paths.singleton_lock()) {
        eprintln!("Kuro engine is already running.");
        std::process::exit(0);
    }
    let policy = kuro_wifi::WifiPolicy::load(&paths.root);
    if policy.disabled {
        kuro_core::release_singleton(&paths.singleton_lock());
        std::process::exit(0);
    }
    let ssid = kuro_wifi::current_ssid();
    let at_home = policy.should_run(ssid.as_deref());
    let force_prewarm = matches!(std::env::var("KURO_STT_PREWARM").unwrap_or_default().as_str(), "1" | "true" | "on");
    if !gate_ci && (at_home || force_prewarm) {
        std::thread::spawn(|| {
            warm_speech();
        });
    }
    let watch_root = paths.root.clone();
    if !gate_ci {
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
    let timing_on = matches!(
        std::env::var("KURO_TIMING").unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    );
    let shared_clock = Arc::new(Mutex::new(TurnClock::default()));
    let voice = Arc::new(Mutex::new(VoiceTurn::new(Timing::new(paths.timing_csv(), timing_on))));
    let capture = Arc::new(Mutex::new(kuro_audio::PttBuffer::new()));
    let capture_stream: Rc<RefCell<Option<kuro_audio::CaptureSession>>> = Rc::new(RefCell::new(None));
    let listening = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let busy = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let mut api_state: Option<Arc<kuro_ipc::AppState>> = None;
    if kuro_core::api_requested() {
        let token = kuro_core::ensure_token(&paths.api_token()).unwrap_or_default();
        if kuro_ipc::can_listen(&token) {
            let state = Arc::new(kuro_ipc::AppState {
                token,
                config: Mutex::new(kuro_core::EngineConfig::default()),
                hud: Mutex::new(kuro_ipc::HudSnap::default()),
                contacts: Mutex::new(paths.contacts()),
                workflows: Mutex::new(paths.workflows()),
                timing: Mutex::new(Timing::new(paths.timing_csv(), timing_on)),
                brain: Mutex::new(kuro_tools::brain::Brain::default()),
                dismiss_hits: Mutex::new(0),
                shutdown_hits: Mutex::new(0),
                shutdown_hook: Mutex::new(Some(Arc::new(unload_speech))),
                clock: shared_clock.clone(),
            });
            api_state = Some(state.clone());
            let port = kuro_core::api_port();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio");
                rt.block_on(async move {
                    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.expect("bind");
                    kuro_ipc::serve(state, listener).await;
                });
            });
        }
    }

    if gate_ci {
        let Some(state) = api_state else {
            eprintln!("KURO_GATE_CI requires KURO_API=on with a valid api_token.");
            kuro_core::release_singleton(&paths.singleton_lock());
            std::process::exit(1);
        };
        for _ in 0..80 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if *state.shutdown_hits.lock().unwrap_or_else(|e| e.into_inner()) > 0 {
                kuro_core::release_singleton(&paths.singleton_lock());
                std::process::exit(0);
            }
        }
        kuro_core::release_singleton(&paths.singleton_lock());
        std::process::exit(1);
    }

    let hud = api_state.clone();
    if let Some(state) = hud.as_ref() {
        set_hud(&state.hud, "idle", "idle", Some(vec![0.0; 11]), "", "");
    }

    let spec = kuro_hotkey::default_hotkey();
    let down_capture = capture.clone();
    let up_capture = capture.clone();
    let down_stream = capture_stream.clone();
    let up_stream = capture_stream.clone();
    let esc_stream = capture_stream.clone();
    let down_state = hud.clone();
    let up_state = hud.clone();
    let esc_state = hud.clone();
    let down_listening = listening.clone();
    let up_listening = listening.clone();
    let esc_listening = listening.clone();
    let up_busy = busy.clone();
    let down_busy = busy.clone();
    let esc_busy = busy.clone();
    let down_voice = voice.clone();
    let up_voice = voice.clone();
    let down_clock = shared_clock.clone();
    let up_clock = shared_clock.clone();
    let esc_clock = shared_clock.clone();
    let held = Rc::new(RefCell::new(false));
    let down_held = held.clone();
    let up_held = held.clone();
    let esc_held = held.clone();

    if let Some(state) = hud.clone() {
        let level_capture = capture.clone();
        let level_flag = listening.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(33));
                if !level_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    continue;
                }
                let mut pcm = level_capture.lock().map(|buf| buf.pcm.clone()).unwrap_or_default();
                if pcm.len() > 3_200 {
                    let start = pcm.len() - 3_200;
                    pcm = pcm[start..].to_vec();
                }
                kuro_audio::normalize_gain(&mut pcm, 0.12);
                let bars = kuro_audio::levels_11(&pcm);
                let levels = bars.into_iter().map(f64::from).collect::<Vec<_>>();
                set_hud(&state.hud, "listening", "listening", Some(levels), "", "");
            }
        });
    }

    kuro_hotkey::register_and_wait(
        spec,
        move || {
            if down_busy.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            {
                let Ok(mut clock) = down_clock.lock() else {
                    return;
                };
                if clock.processing {
                    return;
                }
                let _ = clock.begin_turn();
            }
            *down_held.borrow_mut() = true;
            if let Ok(mut buf) = down_capture.lock() {
                buf.pcm.clear();
                buf.partial_mark = 0;
            }
            if let Ok(mut turn) = down_voice.lock() {
                turn.hotkey_down();
            }
            match kuro_audio::start_ptt_capture(down_capture.clone()) {
                Ok(session) => *down_stream.borrow_mut() = Some(session),
                Err(err) => {
                    log_line(&format!("mic open failed: {err}"));
                    if let Some(state) = down_state.as_ref() {
                        set_hud(&state.hud, "error", &err, Some(vec![0.0; 11]), "", "");
                    }
                    *down_held.borrow_mut() = false;
                    return;
                }
            }
            down_listening.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(state) = down_state.as_ref() {
                set_hud(&state.hud, "listening", "listening", Some(vec![0.0; 11]), "", "");
            }
        },
        move || {
            if !*up_held.borrow() {
                return;
            }
            *up_held.borrow_mut() = false;
            up_listening.store(false, std::sync::atomic::Ordering::Relaxed);
            *up_stream.borrow_mut() = None;
            let epoch = up_clock.lock().map(|c| c.snapshot()).unwrap_or(0);
            if let Ok(mut clock) = up_clock.lock() {
                clock.processing = true;
            }
            let mut pcm = up_capture.lock().map(|buf| buf.pcm.clone()).unwrap_or_default();
            let raw_rms = kuro_audio::rms(&pcm);
            kuro_audio::normalize_gain(&mut pcm, 0.12);
            let energy = kuro_audio::rms(&pcm);
            log_line(&format!(
                "ptt release pcm={} raw_rms={raw_rms:.5} rms={energy:.5} secs={:.2}",
                pcm.len(),
                pcm.len() as f32 / 16_000.0
            ));
            let bars = kuro_audio::levels_11(&pcm);
            let levels = bars.into_iter().map(f64::from).collect::<Vec<_>>();
            if let Some(state) = up_state.as_ref() {
                set_hud(&state.hud, "thinking", "thinking", Some(levels), "", "");
            }
            if up_busy.swap(true, std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let voice = up_voice.clone();
            let state = up_state.clone();
            let clock = up_clock.clone();
            let busy_flag = up_busy.clone();
            std::thread::spawn(move || {
                let prompt = kuro_stt::initial_prompt(&[], &[]);
                let transcript = if pcm.len() < 1600 {
                    String::new()
                } else {
                    match kuro_stt::transcribe_pcm(&pcm, 16_000, false, &prompt) {
                        Ok(text) => text,
                        Err(err) => {
                            log_line(&format!("stt failed: {err}"));
                            if let Some(app) = state.as_ref() {
                                set_hud(&app.hud, "error", &err, Some(vec![0.0; 11]), "", "");
                            }
                            if let Ok(mut c) = clock.lock() {
                                c.processing = false;
                            }
                            busy_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }
                    }
                };
                log_line(&format!("stt text=`{transcript}`"));
                if let Some(app) = state.as_ref() {
                    let label = if transcript.is_empty() { "quiet" } else { "thinking" };
                    set_hud(&app.hud, "thinking", label, None, &transcript, "");
                }
                let neural = ollama_up();
                let you = transcript.clone();
                let hud_state = state.clone();
                let reply = voice
                    .lock()
                    .map(|mut turn| {
                        turn.run(&transcript, &transcript, false, false, epoch, &clock, "fallback", neural, |spoken| {
                            if let Some(app) = hud_state.as_ref() {
                                set_hud(&app.hud, "speaking", "speaking", None, &you, spoken);
                            }
                        })
                    })
                    .unwrap_or_default();
                let still_live = clock.lock().map(|c| c.live(epoch)).unwrap_or(false);
                if let Some(app) = state.as_ref() {
                    if still_live && transcript.is_empty() {
                        set_hud(&app.hud, "error", "quiet", Some(vec![0.0; 11]), "", "");
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    } else if still_live && !reply.is_empty() {
                        // Audio has finished; keep the reply text visible briefly.
                        set_hud(&app.hud, "speaking", "speaking", None, &transcript, &reply);
                        std::thread::sleep(std::time::Duration::from_millis(120));
                    }
                    set_hud(&app.hud, "idle", "idle", Some(vec![0.0; 11]), "", "");
                }
                if let Ok(mut c) = clock.lock() {
                    c.processing = false;
                }
                busy_flag.store(false, std::sync::atomic::Ordering::Relaxed);
            });
        },
        move || {
            esc_listening.store(false, std::sync::atomic::Ordering::Relaxed);
            *esc_stream.borrow_mut() = None;
            *esc_held.borrow_mut() = false;
            if let Ok(mut clock) = esc_clock.lock() {
                clock.force_unblock();
            }
            esc_busy.store(false, std::sync::atomic::Ordering::Relaxed);
            if let Some(state) = esc_state.as_ref() {
                set_hud(&state.hud, "idle", "idle", Some(vec![0.0; 11]), "", "");
            }
            log_line("escape/dismiss");
        },
    );
}
