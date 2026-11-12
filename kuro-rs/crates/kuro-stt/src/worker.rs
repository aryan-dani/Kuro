//! In-process whisper-rs GGML backend. Loads once, drops on Wi-Fi leave or /shutdown.

use std::path::PathBuf;
use std::sync::Mutex;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

struct Model {
    state: WhisperState,
    _ctx: WhisperContext,
}

static MODEL: Mutex<Option<Model>> = Mutex::new(None);

pub fn shutdown() {
    crate::vad::unload();
    let mut slot = match MODEL.lock() {
        Ok(slot) => slot,
        Err(poison) => poison.into_inner(),
    };
    *slot = None;
}

pub fn prewarm(_device: &str) -> Result<(), String> {
    ensure()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..")
}

pub fn ggml_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("KURO_WHISPER_GGML") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let candidate = local.join("Kuro").join("models").join("ggml-small.en.bin");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let bench = repo_root().join("bench").join("models");
    for name in ["ggml-small.en.bin", "small.en.bin", "ggml-base.en.bin"] {
        let candidate = bench.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn load_model() -> Result<Model, String> {
    let path = ggml_path().ok_or_else(|| {
        "Whisper GGML model is missing. Set KURO_WHISPER_GGML or run Build-KuroNative.ps1.".to_string()
    })?;
    let ctx = WhisperContext::new_with_params(path.to_string_lossy().as_ref(), WhisperContextParameters::default())
        .map_err(|err| format!("Could not load Whisper GGML: {err}"))?;
    let state = ctx.create_state().map_err(|err| format!("Whisper state failed: {err}"))?;
    Ok(Model { state, _ctx: ctx })
}

fn ensure() -> Result<(), String> {
    let mut slot = MODEL.lock().map_err(|_| "Whisper model lock failed.".to_string())?;
    if slot.is_some() {
        return Ok(());
    }
    *slot = Some(load_model()?);
    Ok(())
}

/// Drop leading/trailing silence using fixed-frame RMS energy.
/// Whisper needs >= 1 s of audio; pad (or keep the original) when trim is too aggressive.
fn trim_silence_energy(audio: &[f32]) -> Vec<f32> {
    if audio.is_empty() {
        return Vec::new();
    }
    const FRAME: usize = 320; // 20 ms at 16 kHz
    const THRESH: f32 = 0.01;
    const MIN_SPEECH_FRAMES: usize = 3; // 60 ms
    const PAD: usize = 2_400; // 150 ms
    const MIN_SAMPLES: usize = 17_600; // 1.1 s at 16 kHz (Whisper rejects < 1000 ms)

    let frames = audio.len().div_ceil(FRAME);
    let mut energy = Vec::with_capacity(frames);
    for i in 0..frames {
        let start = i * FRAME;
        let end = (start + FRAME).min(audio.len());
        let slice = &audio[start..end];
        let sum: f32 = slice.iter().map(|s| s * s).sum();
        let rms = (sum / slice.len().max(1) as f32).sqrt();
        energy.push(rms >= THRESH);
    }

    let mut first = None;
    let mut run = 0usize;
    for (i, &speech) in energy.iter().enumerate() {
        run = if speech { run + 1 } else { 0 };
        if run >= MIN_SPEECH_FRAMES {
            first = Some(i + 1 - MIN_SPEECH_FRAMES);
            break;
        }
    }
    let Some(first_frame) = first else {
        let max_amp = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        if max_amp < 0.005 {
            return Vec::new();
        }
        return pad_to_min(audio.to_vec(), MIN_SAMPLES);
    };

    let mut last_frame = first_frame;
    run = 0;
    for (i, &speech) in energy.iter().enumerate().rev() {
        run = if speech { run + 1 } else { 0 };
        if run >= MIN_SPEECH_FRAMES {
            last_frame = (i + MIN_SPEECH_FRAMES - 1).min(frames.saturating_sub(1));
            break;
        }
    }

    let start = first_frame.saturating_mul(FRAME).saturating_sub(PAD);
    let end = ((last_frame + 1) * FRAME + PAD).min(audio.len());
    if start >= end {
        return pad_to_min(audio.to_vec(), MIN_SAMPLES);
    }
    pad_to_min(audio[start..end].to_vec(), MIN_SAMPLES)
}

fn pad_to_min(mut audio: Vec<f32>, min_samples: usize) -> Vec<f32> {
    if audio.len() < min_samples {
        audio.resize(min_samples, 0.0);
    }
    audio
}

fn resample_to_16k(pcm: &[f32], sample_rate: u32) -> Vec<f32> {
    if sample_rate == 16_000 || pcm.is_empty() {
        return pcm.to_vec();
    }
    let ratio = sample_rate as f64 / 16_000.0;
    let out_len = ((pcm.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * ratio;
        let lo = src.floor() as usize;
        let hi = (lo + 1).min(pcm.len().saturating_sub(1));
        let frac = (src - lo as f64) as f32;
        out.push(pcm[lo] * (1.0 - frac) + pcm[hi] * frac);
    }
    out
}

pub fn transcribe(
    pcm: &[f32],
    sample_rate: u32,
    _partial: bool,
    prompt: &str,
    _device: &str,
) -> Result<String, String> {
    if pcm.is_empty() {
        return Ok(String::new());
    }
    ensure()?;
    let resampled = resample_to_16k(pcm, sample_rate);
    let audio = crate::vad::trim_best(&resampled, trim_silence_energy);
    if audio.is_empty() {
        return Ok(String::new());
    }
    let mut slot = MODEL.lock().map_err(|_| "Whisper model lock failed.".to_string())?;
    let model = slot.as_mut().ok_or_else(|| "Whisper model is not loaded.".to_string())?;

    let duration_secs = (audio.len() as f32) / 16_000.0;
    // 1500 encoder frames = 30.0s -> 50 frames per second, plus 64-frame safety margin, rounded up to multiple of 64.
    let raw_ctx = ((duration_secs * 50.0) as i32) + 64;
    let audio_ctx = ((raw_ctx + 63) / 64 * 64).clamp(128, 1500);

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(std::thread::available_parallelism().map(|n| (n.get() as i32).clamp(4, 8)).unwrap_or(6));
    params.set_translate(false);
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_no_context(true);
    params.set_single_segment(true);
    params.set_temperature(0.0);
    params.set_temperature_inc(0.0);
    params.set_suppress_blank(true);
    params.set_audio_ctx(audio_ctx);
    if !prompt.trim().is_empty() {
        params.set_initial_prompt(prompt);
    }
    model.state.full(params, &audio).map_err(|err| format!("Whisper inference failed: {err}"))?;
    let n = model.state.full_n_segments().map_err(|err| format!("Whisper segment count failed: {err}"))?;
    let mut text = String::new();
    for i in 0..n {
        if let Ok(segment) = model.state.full_get_segment_text(i) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(segment.trim());
        }
    }
    Ok(text.trim().to_string())
}
