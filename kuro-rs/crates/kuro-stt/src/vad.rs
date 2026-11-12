//! Silero VAD ONNX when present; energy VAD fallback lives in worker.rs.

use std::sync::Mutex;

use ort::value::Tensor;

struct Silero {
    session: ort::session::Session,
}

static SLOT: Mutex<Option<Silero>> = Mutex::new(None);

pub fn unload() {
    if let Ok(mut slot) = SLOT.lock() {
        *slot = None;
    }
}

pub fn ready() -> bool {
    if matches!(
        std::env::var("KURO_DISABLE_SILERO").ok().as_deref().map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    ) {
        return false;
    }
    kuro_core::silero_vad_onnx().is_some()
}

pub fn trim_best(audio: &[f32], energy_fallback: impl FnOnce(&[f32]) -> Vec<f32>) -> Vec<f32> {
    if ready()
        && let Some(trimmed) = trim_silero(audio)
    {
        return trimmed;
    }
    energy_fallback(audio)
}

fn ensure() -> Result<(), String> {
    let mut slot = SLOT.lock().map_err(|_| "silero lock".to_string())?;
    if slot.is_some() {
        return Ok(());
    }
    let path = kuro_core::silero_vad_onnx().ok_or_else(|| "silero missing".to_string())?;
    let session = kuro_core::ort_env::session_from_file(&path)?;
    *slot = Some(Silero { session });
    Ok(())
}

/// Returns speech probability per 512-sample frame @ 16 kHz.
pub fn frame_probs(audio: &[f32]) -> Option<Vec<f32>> {
    ensure().ok()?;
    const FRAME: usize = 512;
    let mut slot = SLOT.lock().ok()?;
    let silero = slot.as_mut()?;
    let mut state = vec![0.0f32; 256];
    let mut probs = Vec::new();
    let mut offset = 0usize;
    while offset + FRAME <= audio.len() {
        let frame = audio[offset..offset + FRAME].to_vec();
        let input = Tensor::from_array(([1usize, FRAME], frame)).ok()?;
        let state_t = Tensor::from_array(([2usize, 1usize, 128usize], state.clone())).ok()?;
        let sr = Tensor::from_array(([] as [usize; 0], vec![16_000i64])).ok()?;
        let outputs = silero
            .session
            .run(ort::inputs![
                "input" => input,
                "state" => state_t,
                "sr" => sr,
            ])
            .ok()?;
        let mut next_state = None;
        let mut prob = None;
        for (name, value) in outputs.iter() {
            let name: &str = name;
            if name == "output"
                && let Ok((_shape, data)) = value.try_extract_tensor::<f32>()
            {
                prob = data.first().copied();
            } else if name == "stateN"
                && let Ok((_shape, data)) = value.try_extract_tensor::<f32>()
            {
                next_state = Some(data.to_vec());
            }
        }
        if let Some(s) = next_state {
            state = s;
        }
        probs.push(prob.unwrap_or(0.0));
        offset += FRAME;
    }
    Some(probs)
}

/// Trim leading/trailing non-speech with 120 ms pad using Silero when available.
pub fn trim_silero(audio: &[f32]) -> Option<Vec<f32>> {
    if audio.is_empty() {
        return Some(Vec::new());
    }
    let probs = frame_probs(audio)?;
    const FRAME: usize = 512;
    const THRESH: f32 = 0.5;
    const PAD: usize = 1_920; // 120 ms @ 16 kHz
    const MIN_SAMPLES: usize = 17_600;

    let mut first = None;
    let mut last = None;
    for (i, &p) in probs.iter().enumerate() {
        if p >= THRESH {
            if first.is_none() {
                first = Some(i);
            }
            last = Some(i);
        }
    }
    let (Some(f), Some(l)) = (first, last) else {
        return None;
    };
    let start = (f * FRAME).saturating_sub(PAD);
    let end = ((l + 1) * FRAME + PAD).min(audio.len());
    if start >= end {
        return None;
    }
    Some(pad_to_min(audio[start..end].to_vec(), MIN_SAMPLES))
}

fn pad_to_min(mut audio: Vec<f32>, min_samples: usize) -> Vec<f32> {
    if audio.len() < min_samples {
        audio.resize(min_samples, 0.0);
    }
    audio
}
