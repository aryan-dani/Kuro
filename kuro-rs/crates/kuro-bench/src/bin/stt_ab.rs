//! Compare recognition variants on saved push-to-talk clips. Local only; never routes tools.
//!
//! Usage: `stt_ab [clips_dir] <truth_file>`
//! - `clips_dir` defaults to `%LOCALAPPDATA%\Kuro\audio` (where the engine saves clips).
//! - `truth_file` has one phrase per line: what you actually said, in the same order as the clips (oldest first).

use std::path::{Path, PathBuf};

use kuro_audio::normalize_gain;
use kuro_bench::{read_wav_f32, wer};

fn default_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    base.join("Kuro").join("audio")
}

fn contact_names() -> Vec<String> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    let Ok(text) = std::fs::read_to_string(base.join("Kuro").join("contacts.json")) else {
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

fn wav_clips(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut clips: Vec<PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|e| e.to_str()) == Some("wav")).collect();
    clips.sort();
    clips
}

/// One recognition setup: whether the contact/command prompt is sent, and whether the clip gets boosted.
struct Variant {
    name: &'static str,
    prompt: bool,
    gain: bool,
}

fn run(variant: &Variant, clip: &Path, prompt: &str) -> Result<String, String> {
    let mut pcm = read_wav_f32(clip)?;
    if variant.gain {
        normalize_gain(&mut pcm, 0.15);
    }
    let prompt = if variant.prompt { prompt } else { "" };
    kuro_stt::transcribe_pcm(&pcm, 16_000, false, prompt)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, truth_path) = match args.as_slice() {
        [truth] => (default_dir(), PathBuf::from(truth)),
        [dir, truth] => (PathBuf::from(dir), PathBuf::from(truth)),
        _ => {
            eprintln!("usage: stt_ab [clips_dir] <truth_file>");
            std::process::exit(2);
        }
    };
    let truth: Vec<String> = std::fs::read_to_string(&truth_path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let clips = wav_clips(&dir);
    let count = clips.len().min(truth.len());
    println!("clips={} truth_lines={} scored={}", clips.len(), truth.len(), count);
    if count == 0 {
        eprintln!("nothing to score: check the clips folder and the truth file");
        std::process::exit(1);
    }
    let prompt = kuro_stt::initial_prompt(&[], &contact_names());
    let variants = [
        Variant { name: "current (prompt + gain)", prompt: true, gain: true },
        Variant { name: "no prompt, gain", prompt: false, gain: true },
        Variant { name: "prompt, no gain", prompt: true, gain: false },
        Variant { name: "no prompt, no gain", prompt: false, gain: false },
    ];
    for variant in &variants {
        let mut total = 0.0;
        println!("\n== {}", variant.name);
        for (clip, expected) in clips.iter().zip(&truth).take(count) {
            let heard = match run(variant, clip, &prompt) {
                Ok(text) => text,
                Err(err) => format!("<error: {err}>"),
            };
            let score = wer(expected, &heard);
            total += score;
            println!("{score:.2}  want=`{expected}`  heard=`{heard}`");
        }
        println!("mean WER {:.3}", total / count as f64);
    }
}
