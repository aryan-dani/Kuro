/// Mic block to a 0..1 bar height. Matches the Python HUD curve.
pub fn voice_level(samples: &[i32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut peak = 0.0f32;
    let mut sum_sq = 0.0f32;
    for sample in samples {
        let v = (*sample as f32).abs();
        peak = peak.max(v);
        sum_sq += v * v;
    }
    let mut audio_peak = peak;
    let n = samples.len() as f32;
    if peak > 1.5 {
        audio_peak /= 32768.0;
        sum_sq /= 32768.0 * 32768.0;
    }
    let rms = (sum_sq / n).sqrt();
    let raw = (rms * 4.2) + (audio_peak * 0.55);
    if raw < 0.008 {
        return (raw * 4.0).min(1.0);
    }
    let shaped = ((raw - 0.008) / 0.22).powf(0.72);
    shaped.clamp(0.0, 1.0)
}
