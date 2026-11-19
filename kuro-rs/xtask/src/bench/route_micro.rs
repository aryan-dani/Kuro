use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use kuro_router::semantic::hash_embed;
use kuro_router::{matches, normalize};

use super::stats::{median_us, p95_us};
use crate::paths::repo_root;

const SAMPLES: &[&str] = &[
    "turn on the desk",
    "turn off the desk",
    "play some music",
    "what time is it",
    "send my clipboard on whatsapp",
    "run workflow morning",
    "daddy is home",
    "cancel",
];

const OUTER_ITERS: u32 = 400;

#[derive(serde::Serialize)]
struct BenchRow {
    op: &'static str,
    median_us: f64,
    p95_us: f64,
    samples: u32,
}

#[derive(serde::Serialize)]
struct BenchReport {
    kind: &'static str,
    phrase_count: usize,
    outer_iters: u32,
    rows: Vec<BenchRow>,
}

pub fn run() -> i32 {
    let normalize_times = time_op("normalize", |text| {
        let _ = normalize(text);
    });
    let match_times = time_op("phrase_match", |text| {
        let _ = matches(text);
    });
    let embed_times = time_op("hash_embed", |text| {
        let t = normalize(text);
        let _ = hash_embed(&t);
    });

    let rows =
        vec![row("normalize", normalize_times), row("phrase_match", match_times), row("hash_embed", embed_times)];

    print_table(&rows);

    let report = BenchReport { kind: "route-micro", phrase_count: SAMPLES.len(), outer_iters: OUTER_ITERS, rows };
    if let Err(e) = write_json(&report) {
        eprintln!("bench: json write skipped: {e}");
    }
    0
}

fn row(op: &'static str, samples: Vec<f64>) -> BenchRow {
    BenchRow { op, median_us: median_us(samples.clone()), p95_us: p95_us(samples), samples: OUTER_ITERS }
}

fn time_op<F>(_: &str, mut op: F) -> Vec<f64>
where
    F: FnMut(&str),
{
    let mut out = Vec::with_capacity(OUTER_ITERS as usize);
    for _ in 0..OUTER_ITERS {
        let start = Instant::now();
        for phrase in SAMPLES {
            op(phrase);
        }
        let elapsed_us = start.elapsed().as_secs_f64() * 1_000_000.0;
        out.push(elapsed_us / SAMPLES.len() as f64);
    }
    out
}

fn print_table(rows: &[BenchRow]) {
    eprintln!("bench router (micro, no models, no daemon)");
    eprintln!("{:16} {:>12} {:>12}", "op", "median_us", "p95_us");
    for r in rows {
        eprintln!("{:16} {:>12.2} {:>12.2}", r.op, r.median_us, r.p95_us);
    }
}

fn write_json(report: &BenchReport) -> std::io::Result<()> {
    let root = repo_root();
    let dir = root.join("kuro-rs").join("target").join("bench");
    fs::create_dir_all(&dir)?;
    let path: PathBuf = dir.join("route-micro.json");
    let body = serde_json::to_string_pretty(report).expect("serialize bench");
    fs::write(path, body)?;
    Ok(())
}
