use std::sync::{Arc, Mutex};

use kuro_core::TurnClock;
use kuro_ipc::AppState;

use crate::speech::ollama_up;
use crate::util::log_line;
use crate::{TurnGate, VoiceTurn, set_hud};

pub(crate) struct PttReleaseJob {
    pub gate_ci: bool,
    pub pcm: Vec<f32>,
    pub epoch: u64,
    pub voice: Arc<Mutex<VoiceTurn>>,
    pub state: Option<Arc<AppState>>,
    pub clock: Arc<Mutex<TurnClock>>,
    pub worker_gate: Arc<TurnGate>,
}

pub(crate) fn spawn_ptt_release_worker(job: PttReleaseJob) {
    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_ptt_release(job)));
    });
}

fn run_ptt_release(job: PttReleaseJob) {
    let PttReleaseJob { gate_ci, pcm, epoch, voice, state, clock, worker_gate } = job;
    let _guard = worker_gate.worker_guard(epoch);
    let book = kuro_tools::whatsapp::address_book();
    let contacts = kuro_tools::contact_bias::stt_contact_names(&book);
    let prompt = kuro_stt::initial_prompt(&[], &contacts);
    let transcript = if pcm.len() < 1600 {
        String::new()
    } else {
        match kuro_stt::transcribe_pcm(&pcm, 16_000, false, &prompt) {
            Ok(text) => kuro_tools::contact_bias::rewrite_transcript(&text, &book),
            Err(err) => {
                log_line(&format!("stt failed: {err}"));
                worker_gate.error_then_idle(&err);
                return;
            }
        }
    };
    log_line(&format!("stt text=`{transcript}`"));
    if let Some(app) = state.as_ref() {
        let label = if transcript.is_empty() { "quiet" } else { "thinking" };
        set_hud(&app.hud, "thinking", label, None, &transcript, "", Some(&app.hud_wake));
    }
    let neural = ollama_up();
    let you = transcript.clone();
    let hud_state = state.clone();
    let reply = voice
        .lock()
        .map(|mut turn| {
            turn.run(&transcript, &transcript, false, false, epoch, &clock, "fallback", neural, |spoken| {
                if let Some(app) = hud_state.as_ref() {
                    set_hud(&app.hud, "speaking", "speaking", None, &you, spoken, Some(&app.hud_wake));
                }
            })
        })
        .unwrap_or_default();
    let still_live = clock.lock().map(|c| c.live(epoch)).unwrap_or(false);
    if let Some(app) = state.as_ref() {
        if still_live && transcript.is_empty() {
            set_hud(&app.hud, "error", "quiet", Some(vec![0.0; 11]), "", "", Some(&app.hud_wake));
            std::thread::sleep(std::time::Duration::from_millis(500));
        } else if still_live && !reply.is_empty() {
            set_hud(&app.hud, "speaking", "speaking", None, &transcript, &reply, Some(&app.hud_wake));
            std::thread::sleep(std::time::Duration::from_millis(250));

            let is_cancel = kuro_router::is_cancel(&kuro_router::normalize(&transcript));
            if !gate_ci && !is_cancel {
                run_follow_ups(&state, &voice, &clock, epoch, neural, app.clone());
            }
        }
        set_hud(&app.hud, "idle", "idle", Some(vec![0.0; 11]), "", "", Some(&app.hud_wake));
    }
}

fn run_follow_ups(
    state: &Option<Arc<AppState>>,
    voice: &Arc<Mutex<VoiceTurn>>,
    clock: &Arc<Mutex<TurnClock>>,
    epoch: u64,
    neural: bool,
    app: Arc<AppState>,
) {
    while clock.lock().map(|c| c.live(epoch)).unwrap_or(false) {
        let (mut follow_buffer, producer) = kuro_audio::PttBuffer::new();
        let session = match kuro_audio::start_ptt_capture(producer) {
            Ok((sess, mic_name)) => {
                log_line(&format!("follow-up mic capture started: {mic_name}"));
                sess
            }
            Err(err) => {
                log_line(&format!("follow-up mic open failed: {err}"));
                break;
            }
        };

        set_hud(&app.hud, "listening", "listening", Some(vec![0.0; 11]), "", "", Some(&app.hud_wake));

        let mut follow = kuro_audio::FollowUpListen {
            max_ms: 16_000,
            silence_ms: 3_500,
            elapsed_ms: 0,
            heard_speech: false,
            quiet_ms: 0,
        };

        let mut frame_count = 0u64;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(33));
            if !clock.lock().map(|c| c.live(epoch)).unwrap_or(false) {
                break;
            }
            follow_buffer.drain_ring();
            let recent_samples = if follow_buffer.pcm.len() > 1600 {
                &follow_buffer.pcm[follow_buffer.pcm.len() - 1600..]
            } else {
                &follow_buffer.pcm[..]
            };
            let frame_rms = kuro_audio::rms(recent_samples);
            let keep_going = follow.tick(frame_rms, 33, true);

            frame_count += 1;
            if frame_count.is_multiple_of(2) {
                let mut pcm_tail = follow_buffer.pcm.clone();
                if pcm_tail.len() > 3_200 {
                    let start = pcm_tail.len() - 3_200;
                    pcm_tail = pcm_tail[start..].to_vec();
                }
                kuro_audio::normalize_gain(&mut pcm_tail, 0.15);
                let bars = kuro_audio::levels_11(&pcm_tail);
                let levels = bars.into_iter().map(f64::from).collect::<Vec<_>>();
                set_hud(&app.hud, "listening", "listening", Some(levels), "", "", Some(&app.hud_wake));
            }

            if !keep_going {
                break;
            }
        }

        drop(session);

        if !clock.lock().map(|c| c.live(epoch)).unwrap_or(false) {
            break;
        }

        if !follow.heard_speech || follow_buffer.pcm.len() < 1600 {
            break;
        }

        set_hud(&app.hud, "thinking", "thinking", Some(vec![0.0; 11]), "", "", Some(&app.hud_wake));
        let mut pcm = follow_buffer.pcm;
        kuro_audio::normalize_gain(&mut pcm, 0.15);
        let book = kuro_tools::whatsapp::address_book();
        let contacts = kuro_tools::contact_bias::stt_contact_names(&book);
        let prompt = kuro_stt::initial_prompt(&[], &contacts);
        let next_transcript = match kuro_stt::transcribe_pcm(&pcm, 16_000, false, &prompt) {
            Ok(text) => kuro_tools::contact_bias::rewrite_transcript(text.trim(), &book),
            Err(err) => {
                log_line(&format!("follow-up stt failed: {err}"));
                break;
            }
        };

        if next_transcript.is_empty() {
            break;
        }

        log_line(&format!("follow-up stt text=`{next_transcript}`"));
        set_hud(&app.hud, "thinking", "thinking", None, &next_transcript, "", Some(&app.hud_wake));

        if kuro_router::is_cancel(&kuro_router::normalize(&next_transcript)) {
            break;
        }

        let you_next = next_transcript.clone();
        let hud_state = state.clone();
        let next_reply = voice
            .lock()
            .map(|mut turn| {
                turn.run(&next_transcript, &next_transcript, false, false, epoch, clock, "fallback", neural, |spoken| {
                    if let Some(app) = hud_state.as_ref() {
                        set_hud(&app.hud, "speaking", "speaking", None, &you_next, spoken, Some(&app.hud_wake));
                    }
                })
            })
            .unwrap_or_default();

        let still_live = clock.lock().map(|c| c.live(epoch)).unwrap_or(false);
        if !still_live || next_reply.is_empty() {
            break;
        }

        set_hud(&app.hud, "speaking", "speaking", None, &next_transcript, &next_reply, Some(&app.hud_wake));
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
