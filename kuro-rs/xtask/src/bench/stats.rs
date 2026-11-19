//! Fixed-sample statistics for bench output (ADR 0014).

pub fn median_us(mut samples: Vec<f64>) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = samples.len() / 2;
    if samples.len().is_multiple_of(2) { (samples[mid - 1] + samples[mid]) / 2.0 } else { samples[mid] }
}

pub fn p95_us(mut samples: Vec<f64>) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((samples.len() as f64) * 0.95).ceil() as usize;
    let idx = idx.saturating_sub(1).min(samples.len() - 1);
    samples[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_odd() {
        assert_eq!(median_us(vec![3.0, 1.0, 2.0]), 2.0);
    }

    #[test]
    fn median_even() {
        assert_eq!(median_us(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
    }

    #[test]
    fn p95_picks_high_tier() {
        let v: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(p95_us(v), 19.0);
    }
}
