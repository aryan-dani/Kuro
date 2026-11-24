//! Replay recorded clips through production STT prep. Does not route tools.

use std::path::PathBuf;
use std::time::Instant;

use kuro_audio::normalize_gain;
use kuro_bench::{EvalSummary, load_labels, read_wav_f32, render_summary, score_clip, summarize, voice_dir};

fn main() {
    let engine = arg_value("--engine").unwrap_or_else(|| "whisper-small-cpu".into());
    let session = arg_value("--session");
    let mut labels = Vec::new();
    for name in ["session1", "session2", "session3"] {
        if let Some(filter) = session.as_deref()
            && !name.ends_with(filter)
            && filter != name
        {
            continue;
        }
        let path = voice_dir().join(name).join("labels.jsonl");
        match load_labels(&path) {
            Ok(rows) => labels.extend(rows),
            Err(err) => println!("skip {}: {err}", path.display()),
        }
    }
    if labels.is_empty() {
        println!("no labels under {}", voice_dir().display());
        println!("record session 1 first. See docs/STT_LLM_EVAL.md");
        return;
    }
    let summaries = match engine_ready(&engine) {
        Ok(()) => run_engine(&engine, &labels),
        Err(reason) => vec![EvalSummary {
            engine: engine.clone(),
            clips: labels.len(),
            command_accuracy: 0.0,
            command_n: 0,
            false_trigger_rate: 0.0,
            negative_n: 0,
            hallucination_rate: 0.0,
            silence_n: 0,
            mean_wer: 0.0,
            wer_n: 0,
            warm_p50_ms: 0.0,
            warm_p95_ms: 0.0,
            cold_load_ms: 0.0,
            peak_vram_mib: sample_vram(),
            skipped: true,
            skip_reason: reason,
        }],
    };
    let table = render_summary(&summaries);
    println!("{table}");
    let out_dir = voice_dir();
    let _ = std::fs::create_dir_all(&out_dir);
    let md = out_dir.join(format!("eval-{engine}.md"));
    let json = out_dir.join(format!("eval-{engine}.json"));
    let _ = std::fs::write(&md, &table);
    let rows: Vec<_> = summaries.iter().map(summary_json).collect();
    if let Ok(body) = serde_json::to_string_pretty(&rows) {
        let _ = std::fs::write(&json, body);
    }
    println!("wrote {}", md.display());
}

fn run_engine(engine: &str, labels: &[kuro_bench::ClipLabel]) -> Vec<EvalSummary> {
    let prompt = kuro_stt::initial_prompt(&[], &contact_names());
    let mut tagged = Vec::new();
    let mut warm = Vec::new();
    let mut warm_by: Vec<(String, f64)> = Vec::new();
    let mut cold_load_ms = 0.0;
    let mut cold_taken = false;
    let mut peak = sample_vram();
    for (index, label) in labels.iter().enumerate() {
        let path = clip_wav(label);
        let Ok(mut pcm) = read_wav_f32(&path) else {
            println!("missing wav {}", path.display());
            continue;
        };
        normalize_gain(&mut pcm, 0.15);
        let mut hyp = String::new();
        let mut last_ms = 0.0;
        for run in 0..3 {
            let started = Instant::now();
            hyp = match kuro_stt::transcribe_pcm(&pcm, 16_000, false, &prompt) {
                Ok(text) => text,
                Err(err) => return vec![skipped(engine, &err)],
            };
            last_ms = started.elapsed().as_secs_f64() * 1000.0;
            if !cold_taken {
                cold_load_ms = last_ms;
                cold_taken = true;
                continue;
            }
            if run > 0 {
                warm.push(last_ms);
                warm_by.push((label.session.clone(), last_ms));
            }
        }
        println!("[{}/{}] {} {:.0} ms", index + 1, labels.len(), label.id, last_ms);
        tagged.push((
            label.session.clone(),
            score_clip(&label.id, &label.kind, &label.expected_text, &label.expected_intent, &hyp),
        ));
        if let Some(now) = sample_vram() {
            peak = Some(peak.unwrap_or(0.0).max(now));
        }
    }
    let all: Vec<_> = tagged.iter().map(|(_, clip)| clip.clone()).collect();
    let mut rows = vec![summarize(engine, &all, &warm, cold_load_ms, peak)];
    for session in ["1", "2", "3"] {
        let subset: Vec<_> = tagged.iter().filter(|(s, _)| s == session).map(|(_, clip)| clip.clone()).collect();
        if subset.is_empty() {
            continue;
        }
        let subset_warm: Vec<f64> = warm_by.iter().filter(|(s, _)| s == session).map(|(_, ms)| *ms).collect();
        rows.push(summarize(&format!("{engine}-s{session}"), &subset, &subset_warm, cold_load_ms, peak));
    }
    rows
}

fn skipped(engine: &str, reason: &str) -> EvalSummary {
    EvalSummary {
        engine: engine.into(),
        clips: 0,
        command_accuracy: 0.0,
        command_n: 0,
        false_trigger_rate: 0.0,
        negative_n: 0,
        hallucination_rate: 0.0,
        silence_n: 0,
        mean_wer: 0.0,
        wer_n: 0,
        cold_load_ms: 0.0,
        warm_p50_ms: 0.0,
        warm_p95_ms: 0.0,
        peak_vram_mib: None,
        skipped: true,
        skip_reason: reason.into(),
    }
}

fn engine_ready(engine: &str) -> Result<(), String> {
    match engine {
        "whisper-small-cpu" => {
            if kuro_stt::ggml_path().is_some() {
                Ok(())
            } else {
                Err("ggml-small.en.bin is missing".into())
            }
        }
        "whisper-turbo-cuda" => {
            if !cfg!(feature = "cuda") {
                return Err("rebuild eval with --features cuda".into());
            }
            if kuro_stt::ggml_path().is_some() {
                Ok(())
            } else {
                Err("turbo ggml file is missing".into())
            }
        }
        "parakeet-v2-cpu" | "parakeet-v3-cpu" | "parakeet-cuda" => {
            Err("no Rust Parakeet runtime in this build. sherpa and transcribe-rs would pull a second onnx runtime beside ort 2.0.0-rc.15.".into())
        }
        other => Err(format!("unknown engine {other}")),
    }
}

fn clip_wav(label: &kuro_bench::ClipLabel) -> PathBuf {
    voice_dir().join(format!("session{}", label.session)).join(format!("{}.wav", label.id))
}

fn sample_vram() -> Option<f64> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next()?.trim().parse().ok()
}

fn summary_json(row: &EvalSummary) -> serde_json::Value {
    serde_json::json!({
        "engine": row.engine,
        "clips": row.clips,
        "command_accuracy": row.command_accuracy,
        "command_n": row.command_n,
        "false_trigger_rate": row.false_trigger_rate,
        "negative_n": row.negative_n,
        "hallucination_rate": row.hallucination_rate,
        "silence_n": row.silence_n,
        "mean_wer": row.mean_wer,
        "cold_load_ms": row.cold_load_ms,
        "warm_p50_ms": row.warm_p50_ms,
        "warm_p95_ms": row.warm_p95_ms,
        "peak_vram_mib": row.peak_vram_mib,
        "skipped": row.skipped,
        "skip_reason": row.skip_reason,
    })
}

fn contact_names() -> Vec<String> {
    let Some(root) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    let path = PathBuf::from(root).join("Kuro").join("contacts.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    value
        .as_object()
        .map(|map| map.keys().filter(|k| !k.eq_ignore_ascii_case("me")).cloned().collect())
        .unwrap_or_default()
}

fn arg_value(flag: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next();
        }
        if let Some(v) = arg.strip_prefix(&format!("{flag}=")) {
            return Some(v.to_string());
        }
    }
    None
}
