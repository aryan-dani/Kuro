//! In-process whisper-rs GGML backend. Loads once, drops on Wi-Fi leave or /shutdown.

use std::path::PathBuf;
use std::sync::Mutex;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

struct Model {
    ctx: WhisperContext,
}

static MODEL: Mutex<Option<Model>> = Mutex::new(None);

pub fn shutdown() {
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
    Ok(Model { ctx })
}

fn ensure() -> Result<(), String> {
    let mut slot = MODEL.lock().map_err(|_| "Whisper model lock failed.".to_string())?;
    if slot.is_some() {
        return Ok(());
    }
    *slot = Some(load_model()?);
    Ok(())
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

pub fn transcribe(pcm: &[f32], sample_rate: u32, partial: bool, prompt: &str, _device: &str) -> Result<String, String> {
    if pcm.is_empty() {
        return Ok(String::new());
    }
    ensure()?;
    let audio = resample_to_16k(pcm, sample_rate);
    let mut slot = MODEL.lock().map_err(|_| "Whisper model lock failed.".to_string())?;
    let model = slot.as_mut().ok_or_else(|| "Whisper model is not loaded.".to_string())?;
    let mut state = model.ctx.create_state().map_err(|err| format!("Whisper state failed: {err}"))?;
    let beam = crate::beam_size(partial);
    let mut params = if beam <= 1 {
        FullParams::new(SamplingStrategy::Greedy { best_of: 1 })
    } else {
        FullParams::new(SamplingStrategy::BeamSearch { beam_size: beam as i32, patience: -1.0 })
    };
    params.set_n_threads(std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4).min(8));
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_single_segment(true);
    if !prompt.trim().is_empty() {
        params.set_initial_prompt(prompt);
    }
    state.full(params, &audio).map_err(|err| format!("Whisper inference failed: {err}"))?;
    let n = state.full_n_segments().map_err(|err| format!("Whisper segment count failed: {err}"))?;
    let mut text = String::new();
    for i in 0..n {
        if let Ok(segment) = state.full_get_segment_text(i) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(segment.trim());
        }
    }
    Ok(text.trim().to_string())
}
