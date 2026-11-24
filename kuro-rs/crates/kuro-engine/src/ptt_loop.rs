use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use kuro_core::TurnClock;
use kuro_ipc::AppState;

use crate::ptt_release::{PttReleaseJob, spawn_ptt_release_worker};
use crate::util::log_line;
use crate::{TurnGate, VoiceTurn, set_hud};

pub struct PttArms {
    pub gate_ci: bool,
    pub hud: Option<Arc<AppState>>,
    pub capture: Arc<Mutex<kuro_audio::PttBuffer>>,
    pub capture_stream: Rc<RefCell<Option<kuro_audio::CaptureSession>>>,
    pub listening: Arc<std::sync::atomic::AtomicBool>,
    pub gate: Arc<TurnGate>,
    pub voice: Arc<Mutex<VoiceTurn>>,
    pub clock: Arc<Mutex<TurnClock>>,
}

fn spawn_level_publisher(
    hud: Arc<AppState>,
    capture: Arc<Mutex<kuro_audio::PttBuffer>>,
    listening: Arc<std::sync::atomic::AtomicBool>,
) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(33));
            if !listening.load(Ordering::Relaxed) {
                continue;
            }
            let Some(mut buf) = capture.lock().ok() else {
                continue;
            };
            buf.drain_ring();
            let mut pcm = buf.pcm.clone();
            if pcm.len() > 3_200 {
                let start = pcm.len() - 3_200;
                pcm = pcm[start..].to_vec();
            }
            kuro_audio::normalize_gain(&mut pcm, 0.15);
            let bars = kuro_audio::levels_11(&pcm);
            buf.hud_levels.store(bars);
            let levels = bars.into_iter().map(f64::from).collect::<Vec<_>>();
            set_hud(&hud.hud, "listening", "listening", Some(levels), "", "", Some(&hud.hud_wake));
        }
    });
}

pub fn arm_ptt_hotkey(arms: PttArms, singleton_lock: &std::path::Path) {
    if let Some(state) = arms.hud.clone() {
        spawn_level_publisher(state, arms.capture.clone(), arms.listening.clone());
    }

    let PttArms { gate_ci, hud, capture, capture_stream, listening, gate, voice, clock: shared_clock } = arms;

    let spec = kuro_hotkey::default_hotkey();
    let down_capture = capture.clone();
    let up_capture = capture.clone();
    let down_stream = capture_stream.clone();
    let up_stream = capture_stream.clone();
    let esc_stream = capture_stream.clone();
    let down_state = hud.clone();
    let up_state = hud.clone();
    let down_listening = listening.clone();
    let up_listening = listening.clone();
    let esc_listening = listening.clone();
    let down_gate = gate.clone();
    let up_gate = gate.clone();
    let esc_gate = gate.clone();
    let down_voice = voice.clone();
    let up_voice = voice.clone();
    let down_clock = shared_clock.clone();
    let up_clock = shared_clock.clone();
    let held = Rc::new(RefCell::new(false));
    let down_held = held.clone();
    let up_held = held.clone();
    let esc_held = held.clone();

    if let Err(err) = kuro_hotkey::register_and_wait(
        spec,
        move || {
            if down_gate.press_blocked() {
                return;
            }
            {
                let Ok(mut clock) = down_clock.lock() else {
                    return;
                };
                let _ = clock.begin_turn();
            }
            *down_held.borrow_mut() = true;
            let producer = down_capture.lock().ok().map(|mut buf| buf.reset());
            if let Ok(mut turn) = down_voice.lock() {
                turn.hotkey_down(&down_clock);
            }
            let Some(producer) = producer else {
                return;
            };
            match kuro_audio::start_ptt_capture(producer) {
                Ok((session, mic_name)) => {
                    log_line(&format!("mic capture started: {mic_name}"));
                    *down_stream.borrow_mut() = Some(session);
                }
                Err(err) => {
                    log_line(&format!("mic open failed: {err}"));
                    if let Some(state) = down_state.as_ref() {
                        set_hud(&state.hud, "error", &err, Some(vec![0.0; 11]), "", "", Some(&state.hud_wake));
                    }
                    *down_held.borrow_mut() = false;
                    return;
                }
            }
            down_listening.store(true, Ordering::Relaxed);
            if let Some(state) = down_state.as_ref() {
                set_hud(&state.hud, "listening", "listening", Some(vec![0.0; 11]), "", "", Some(&state.hud_wake));
            }
        },
        move || {
            if !*up_held.borrow() {
                return;
            }
            *up_held.borrow_mut() = false;
            up_listening.store(false, Ordering::Relaxed);
            *up_stream.borrow_mut() = None;
            let epoch = up_clock.lock().map(|c| c.snapshot()).unwrap_or(0);
            up_gate.set_processing();
            let mut pcm = up_capture
                .lock()
                .map(|mut buf| {
                    buf.drain_ring();
                    buf.pcm.clone()
                })
                .unwrap_or_default();
            let raw_rms = kuro_audio::rms(&pcm);
            let clip = kuro_audio::clip::save_clip(&kuro_core::KuroPaths::from_env().root.join("audio"), &pcm);
            kuro_audio::normalize_gain(&mut pcm, 0.15);
            let energy = kuro_audio::rms(&pcm);
            log_line(&format!(
                "ptt release pcm={} raw_rms={raw_rms:.5} rms={energy:.5} secs={:.2}",
                pcm.len(),
                pcm.len() as f32 / 16_000.0
            ));
            let bars = kuro_audio::levels_11(&pcm);
            let levels = bars.into_iter().map(f64::from).collect::<Vec<_>>();
            if let Some(state) = up_state.as_ref() {
                set_hud(&state.hud, "thinking", "thinking", Some(levels), "", "", Some(&state.hud_wake));
            }
            if !up_gate.try_claim_worker() {
                if let Ok(mut clock) = up_clock.lock() {
                    clock.processing = false;
                }
                return;
            }
            spawn_ptt_release_worker(PttReleaseJob {
                gate_ci,
                pcm,
                clip,
                epoch,
                voice: up_voice.clone(),
                state: up_state.clone(),
                clock: up_clock.clone(),
                worker_gate: up_gate.clone(),
            });
        },
        move || {
            let was_held = *esc_held.borrow();
            let was_listening = esc_listening.load(Ordering::Relaxed);
            if !(was_held || was_listening || esc_gate.press_blocked()) {
                return;
            }
            esc_listening.store(false, Ordering::Relaxed);
            *esc_stream.borrow_mut() = None;
            *esc_held.borrow_mut() = false;
            esc_gate.cancel();
            log_line("escape/dismiss");
        },
    ) {
        log_line(&format!("hotkey failed: {err}"));
        kuro_core::release_singleton(singleton_lock);
        std::process::exit(1);
    }
}
