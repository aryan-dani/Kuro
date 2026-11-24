use kuro_router::classify_intent;

#[derive(Clone, Debug, PartialEq)]
pub struct ClipResult {
    pub id: String,
    pub kind: String,
    pub expected_intent: String,
    pub hyp_intent: String,
    pub intent_ok: bool,
    pub wer: f64,
    pub hyp_empty: bool,
    pub ref_empty: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvalSummary {
    pub engine: String,
    pub clips: usize,
    pub command_accuracy: f64,
    pub command_n: usize,
    pub false_trigger_rate: f64,
    pub negative_n: usize,
    pub hallucination_rate: f64,
    pub silence_n: usize,
    pub mean_wer: f64,
    pub wer_n: usize,
    pub cold_load_ms: f64,
    pub warm_p50_ms: f64,
    pub warm_p95_ms: f64,
    pub peak_vram_mib: Option<f64>,
    pub skipped: bool,
    pub skip_reason: String,
}

pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let r: Vec<&str> = reference.split_whitespace().collect();
    let h: Vec<&str> = hypothesis.split_whitespace().collect();
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    let mut cur = vec![0; h.len() + 1];
    for (i, rw) in r.iter().enumerate() {
        cur[0] = i + 1;
        for (j, hw) in h.iter().enumerate() {
            let cost = if rw.eq_ignore_ascii_case(hw) { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[h.len()] as f64 / r.len() as f64
}

pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn score_clip(id: &str, kind: &str, expected_text: &str, expected_intent: &str, hypothesis: &str) -> ClipResult {
    let hyp_intent = classify_intent(hypothesis).to_string();
    let intent_ok = hyp_intent == expected_intent;
    ClipResult {
        id: id.to_string(),
        kind: kind.to_string(),
        expected_intent: expected_intent.to_string(),
        hyp_intent,
        intent_ok,
        wer: wer(expected_text, hypothesis),
        hyp_empty: hypothesis.trim().is_empty(),
        ref_empty: expected_text.trim().is_empty(),
    }
}

pub fn summarize(
    engine: &str,
    clips: &[ClipResult],
    warm_ms: &[f64],
    cold_load_ms: f64,
    peak_vram_mib: Option<f64>,
) -> EvalSummary {
    let commands: Vec<&ClipResult> = clips.iter().filter(|c| c.kind == "command" || c.kind == "tool").collect();
    let negatives: Vec<&ClipResult> = clips.iter().filter(|c| c.kind == "negative").collect();
    let silence: Vec<&ClipResult> =
        clips.iter().filter(|c| matches!(c.kind.as_str(), "silence" | "noise" | "short")).collect();
    let wer_clips: Vec<&ClipResult> = clips.iter().filter(|c| !c.ref_empty).collect();
    let command_ok = commands.iter().filter(|c| c.intent_ok).count();
    let false_triggers = negatives.iter().filter(|c| c.hyp_intent != "none").count();
    let hallucinations = silence.iter().filter(|c| !c.hyp_empty).count();
    let mut warm = warm_ms.to_vec();
    warm.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    EvalSummary {
        engine: engine.to_string(),
        clips: clips.len(),
        command_accuracy: ratio(command_ok, commands.len()),
        command_n: commands.len(),
        false_trigger_rate: ratio(false_triggers, negatives.len()),
        negative_n: negatives.len(),
        hallucination_rate: ratio(hallucinations, silence.len()),
        silence_n: silence.len(),
        mean_wer: if wer_clips.is_empty() {
            0.0
        } else {
            wer_clips.iter().map(|c| c.wer).sum::<f64>() / wer_clips.len() as f64
        },
        wer_n: wer_clips.len(),
        cold_load_ms,
        warm_p50_ms: percentile(&warm, 0.50),
        warm_p95_ms: percentile(&warm, 0.95),
        peak_vram_mib,
        skipped: false,
        skip_reason: String::new(),
    }
}

fn ratio(num: usize, den: usize) -> f64 {
    if den == 0 { 0.0 } else { num as f64 / den as f64 }
}

/// Aggregate table only. No clip text.
pub fn render_summary(rows: &[EvalSummary]) -> String {
    let mut out = String::from(
        "| engine | clips | command acc | false trigger | hallucination | mean WER | warm p50 ms | warm p95 ms | vram MiB |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        if row.skipped {
            out.push_str(&format!("| {} | skipped |  |  |  |  |  |  | {} |\n", row.engine, row.skip_reason));
            continue;
        }
        let vram = row.peak_vram_mib.map(|v| format!("{v:.0}")).unwrap_or_else(|| "n/a".into());
        out.push_str(&format!(
            "| {} | {} | {:.3} (n={}) | {:.3} (n={}) | {:.3} (n={}) | {:.3} | {:.0} | {:.0} | {} |\n",
            row.engine,
            row.clips,
            row.command_accuracy,
            row.command_n,
            row.false_trigger_rate,
            row.negative_n,
            row.hallucination_rate,
            row.silence_n,
            row.mean_wer,
            row.warm_p50_ms,
            row.warm_p95_ms,
            vram
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wer_counts_one_substitution() {
        assert!((wer("desk on", "desk off") - 0.5).abs() < 1e-9);
        assert!((wer("desk on", "desk on") - 0.0).abs() < 1e-9);
    }

    #[test]
    fn negative_question_is_not_a_false_trigger_when_hyp_is_none() {
        let clip = score_clip(
            "n1",
            "negative",
            "why does my monitor turn off when I play games",
            "none",
            "why does my monitor turn off when I play games",
        );
        assert_eq!(clip.hyp_intent, "none");
        assert!(clip.intent_ok);
    }

    #[test]
    fn summary_false_trigger_rate() {
        let clips = vec![
            score_clip("c", "command", "desk on", "power_on", "desk on"),
            score_clip("n", "negative", "why does my monitor turn off when I play games", "none", "desk off"),
            score_clip("s", "silence", "", "none", "hello"),
        ];
        let sum = summarize("whisper-small-cpu", &clips, &[100.0, 200.0, 300.0], 10.0, None);
        assert!((sum.command_accuracy - 1.0).abs() < 1e-9);
        assert!((sum.false_trigger_rate - 1.0).abs() < 1e-9);
        assert!((sum.hallucination_rate - 1.0).abs() < 1e-9);
        assert!(render_summary(&[sum]).contains("whisper-small-cpu"));
    }
}
