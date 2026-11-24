//! Streaming anti-aliased resampler for mic capture (device rate down to 16 kHz).
//!
//! Linear interpolation alone folds everything above the output Nyquist (8 kHz at 16 kHz)
//! back into the speech band, which smears sibilants such as "s". This uses a Hann-windowed
//! sinc low-pass at the output Nyquist and keeps its history between capture callbacks.

use std::f64::consts::PI;

/// Half-width of the sinc kernel in input samples (48 taps total at 48 kHz -> 16 kHz).
const HALF: i64 = 48;

pub struct Resampler {
    from: u32,
    to: u32,
    ratio: f64,
    cutoff: f64,
    hist: Vec<f32>,
    /// Absolute input index of `hist[0]`.
    base: i64,
    /// Absolute index of the next output sample.
    next_out: u64,
}

impl Resampler {
    pub fn new(from_hz: u32, to_hz: u32) -> Self {
        let from = from_hz.max(1);
        let to = to_hz.max(1);
        // Stored so `process` can pass through when no conversion is needed.
        let ratio = f64::from(from) / f64::from(to);
        // Cutoff in cycles per input sample, just under the output Nyquist.
        let cutoff = 0.45 * (f64::from(to) / f64::from(from)).min(1.0);
        Self { from, to, ratio, cutoff, hist: Vec::new(), base: 0, next_out: 0 }
    }

    /// Feed the next block of mono samples at the input rate; returns the output samples ready so far.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.from == self.to {
            return input.to_vec();
        }
        self.hist.extend_from_slice(input);
        let mut out = Vec::new();
        loop {
            let t = self.next_out as f64 * self.ratio;
            let center = t.floor() as i64;
            if center + HALF >= self.base + self.hist.len() as i64 {
                break;
            }
            out.push(self.sample_at(t, center));
            self.next_out += 1;
        }
        let keep_from = (self.next_out as f64 * self.ratio).floor() as i64 - HALF;
        let drop = (keep_from - self.base).clamp(0, self.hist.len() as i64) as usize;
        self.hist.drain(..drop);
        self.base += drop as i64;
        out
    }

    fn sample_at(&self, t: f64, center: i64) -> f32 {
        let mut acc = 0.0f64;
        let mut weight_sum = 0.0f64;
        for n in (center - HALF + 1)..=(center + HALF) {
            let idx = n - self.base;
            let x = if idx >= 0 && (idx as usize) < self.hist.len() { f64::from(self.hist[idx as usize]) } else { 0.0 };
            let w = kernel(t - n as f64, self.cutoff);
            acc += x * w;
            weight_sum += w;
        }
        if weight_sum.abs() < 1e-12 { 0.0 } else { (acc / weight_sum) as f32 }
    }
}

/// Hann-windowed sinc low-pass impulse response at distance `d` (input samples).
fn kernel(d: f64, cutoff: f64) -> f64 {
    let window_pos = d / HALF as f64;
    if window_pos.abs() >= 1.0 {
        return 0.0;
    }
    let x = 2.0 * cutoff * d;
    let sinc = if x.abs() < 1e-9 { 1.0 } else { (PI * x).sin() / (PI * x) };
    let hann = 0.5 * (1.0 + (PI * window_pos).cos());
    2.0 * cutoff * sinc * hann
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, rate: u32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n).map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin()).collect()
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()))
    }

    #[test]
    fn speech_band_tone_keeps_its_level() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(1_000.0, 48_000, 1.0));
        let settled = &out[200..];
        assert!((peak(settled) - 1.0).abs() < 0.05, "1 kHz peak {}", peak(settled));
    }

    #[test]
    fn tone_above_the_16k_band_is_removed_not_folded() {
        // 10 kHz is above the 8 kHz Nyquist of 16 kHz output. Linear decimation folds it back into speech.
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&sine(10_000.0, 48_000, 1.0));
        let settled = &out[200..];
        assert!(peak(settled) < 0.05, "10 kHz leaked through at {}", peak(settled));
    }

    #[test]
    fn chunked_input_matches_one_shot() {
        let input = sine(3_000.0, 48_000, 0.5);
        let mut whole = Resampler::new(48_000, 16_000);
        let expected = whole.process(&input);
        let mut chunked = Resampler::new(48_000, 16_000);
        let mut got = Vec::new();
        for chunk in input.chunks(480) {
            got.extend(chunked.process(chunk));
        }
        assert_eq!(got.len(), expected.len());
        let worst = got.iter().zip(&expected).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "chunk boundary error {worst}");
    }

    #[test]
    fn one_second_at_48k_gives_about_one_second_at_16k() {
        let mut resampler = Resampler::new(48_000, 16_000);
        let out = resampler.process(&vec![0.0; 48_000]);
        assert!((out.len() as i64 - 16_000).abs() < 60, "got {} samples", out.len());
    }

    #[test]
    fn same_rate_is_passed_through() {
        let mut resampler = Resampler::new(16_000, 16_000);
        let input = sine(500.0, 16_000, 0.1);
        assert_eq!(resampler.process(&input), input);
    }
}
