//! Microphone capture helpers: resample, 11 HUD bars, push-to-talk, follow-up VAD.

#[derive(Clone, Debug)]
pub struct FollowUpListen {
    pub max_ms: u64,
    pub silence_ms: u64,
    pub elapsed_ms: u64,
    pub heard_speech: bool,
    pub quiet_ms: u64,
}

impl Default for FollowUpListen {
    fn default() -> Self {
        Self { max_ms: 7_000, silence_ms: 700, elapsed_ms: 0, heard_speech: false, quiet_ms: 0 }
    }
}

impl FollowUpListen {
    /// Returns true while the mic should stay open.
    pub fn tick(&mut self, rms: f32, frame_ms: u64, live: bool) -> bool {
        if !live {
            return false;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(frame_ms);
        if self.elapsed_ms >= self.max_ms {
            return false;
        }
        if rms >= 0.02 {
            self.heard_speech = true;
            self.quiet_ms = 0;
        } else if self.heard_speech {
            self.quiet_ms = self.quiet_ms.saturating_add(frame_ms);
            if self.quiet_ms >= self.silence_ms {
                return false;
            }
        }
        true
    }
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

pub fn to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved.chunks(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect()
}

pub fn resample_linear(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if input.is_empty() || from_hz == 0 || to_hz == 0 {
        return Vec::new();
    }
    if from_hz == to_hz {
        return input.to_vec();
    }
    let out_len = ((input.len() as u64) * u64::from(to_hz) / u64::from(from_hz)).max(1) as usize;
    let mut out = Vec::with_capacity(out_len);
    let scale = from_hz as f32 / to_hz as f32;
    for i in 0..out_len {
        let pos = i as f32 * scale;
        let idx = pos.floor() as usize;
        let frac = pos - idx as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

pub fn to_16k_mono(interleaved: &[f32], channels: usize, sample_rate: u32) -> Vec<f32> {
    resample_linear(&to_mono(interleaved, channels), sample_rate, 16_000)
}

/// Eleven bar heights in `0.0..=1.0` for the `/events` frame.
/// Quiet laptop mics often sit around rms 0.002–0.01; gain keeps the HUD alive.
pub fn levels_11(samples: &[f32]) -> [f32; 11] {
    let mut bars = [0.0f32; 11];
    if samples.is_empty() {
        return bars;
    }
    let chunk = (samples.len() / 11).max(1);
    for (i, bar) in bars.iter_mut().enumerate() {
        let start = i * chunk;
        if start >= samples.len() {
            break;
        }
        let end = if i == 10 { samples.len() } else { (start + chunk).min(samples.len()) };
        let energy = rms(&samples[start..end]) * 55.0;
        *bar = energy.clamp(0.0, 1.0);
    }
    bars
}

/// Boost quiet laptop mics toward a target RMS before STT / HUD.
pub fn normalize_gain(pcm: &mut [f32], target_rms: f32) {
    let current = rms(pcm);
    if current < 1e-6 || target_rms <= 0.0 {
        return;
    }
    let gain = (target_rms / current).clamp(1.0, 24.0);
    for sample in pcm.iter_mut() {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

#[derive(Clone, Debug)]
pub struct PttBuffer {
    pub pcm: Vec<f32>,
    pub partial_mark: usize,
}

impl PttBuffer {
    pub fn new() -> Self {
        Self { pcm: Vec::new(), partial_mark: 0 }
    }

    pub fn push(&mut self, frame: &[f32]) {
        self.pcm.extend_from_slice(frame);
    }

    /// True once another 0.8 s of 16 kHz audio is ready for a partial transcript.
    pub fn take_partial(&mut self) -> Option<&[f32]> {
        const WINDOW: usize = 12_800;
        if self.pcm.len().saturating_sub(self.partial_mark) < WINDOW {
            return None;
        }
        self.partial_mark = self.pcm.len();
        Some(&self.pcm)
    }
}

impl Default for PttBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn open_input() -> Result<cpal::Device, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let preferred = ["microphone array", "realtek", "microphone"];
    let banned = ["hands-free", "stereo mix", "what u hear", "cable"];
    if let Ok(devices) = host.input_devices() {
        let mut ranked: Vec<(i32, cpal::Device)> = Vec::new();
        for device in devices {
            let name = device.name().unwrap_or_default().to_ascii_lowercase();
            if banned.iter().any(|part| name.contains(part)) {
                continue;
            }
            let score = preferred
                .iter()
                .enumerate()
                .find_map(|(idx, part)| name.contains(part).then_some(100 - idx as i32))
                .unwrap_or(1);
            ranked.push((score, device));
        }
        ranked.sort_by_key(|a| std::cmp::Reverse(a.0));
        if let Some((_, device)) = ranked.into_iter().next() {
            return Ok(device);
        }
    }
    host.default_input_device().ok_or_else(|| "No microphone is available.".into())
}

/// Owns a live microphone stream. Drop it to stop capture.
pub struct CaptureSession {
    _stream: cpal::Stream,
}

fn push_f32(
    buffer: &std::sync::Arc<std::sync::Mutex<PttBuffer>>,
    interleaved: &[f32],
    channels: usize,
    sample_rate: u32,
) {
    let mono = to_16k_mono(interleaved, channels, sample_rate);
    if let Ok(mut slot) = buffer.lock() {
        slot.push(&mono);
    }
}

/// Start a default-input stream that appends 16 kHz mono f32 into `buffer`.
pub fn start_ptt_capture(buffer: std::sync::Arc<std::sync::Mutex<PttBuffer>>) -> Result<CaptureSession, String> {
    use cpal::SampleFormat;
    use cpal::traits::{DeviceTrait, StreamTrait};

    let device = open_input()?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".into());
    eprintln!("Kuro microphone: {device_name}");
    let supported = device.default_input_config().map_err(|err| format!("Microphone config failed: {err}"))?;
    let sample_rate = supported.sample_rate().0;
    let channels = usize::from(supported.channels().max(1));
    let format = supported.sample_format();
    eprintln!("Kuro mic format={format:?} rate={sample_rate} ch={channels}");
    let config: cpal::StreamConfig = supported.into();
    let err_fn = |err| eprintln!("Kuro microphone stream error: {err}");

    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            {
                let buffer = buffer.clone();
                move |data: &[f32], _| push_f32(&buffer, data, channels, sample_rate)
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            {
                let buffer = buffer.clone();
                move |data: &[i16], _| {
                    let floats: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                    push_f32(&buffer, &floats, channels, sample_rate);
                }
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            {
                let buffer = buffer.clone();
                move |data: &[u16], _| {
                    let floats: Vec<f32> = data.iter().map(|s| (*s as f32 / u16::MAX as f32) * 2.0 - 1.0).collect();
                    push_f32(&buffer, &floats, channels, sample_rate);
                }
            },
            err_fn,
            None,
        ),
        other => {
            return Err(format!("Unsupported microphone sample format: {other}"));
        }
    }
    .map_err(|err| format!("Could not open the microphone: {err}"))?;

    stream.play().map_err(|err| format!("Could not start the microphone: {err}"))?;
    Ok(CaptureSession { _stream: stream })
}
