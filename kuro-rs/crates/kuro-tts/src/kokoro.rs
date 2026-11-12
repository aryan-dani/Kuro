//! Direct Kokoro-82M ONNX via workspace-pinned `ort` (no kokoro-en).

#[path = "kokoro_vocab.rs"]
mod kokoro_vocab;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn silent_cmd<P: AsRef<std::ffi::OsStr>>(prog: P) -> Command {
    let mut cmd = Command::new(prog);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd
}

use ort::value::Tensor;

use self::kokoro_vocab::kokoro_char_id;

const STYLE_DIM: usize = 256;
const MAX_PHONEMES: usize = 510;
const KOKORO_HZ: u32 = 24_000;
const PLAYBACK_HZ: u32 = 22_050;

pub fn model_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(path) = std::env::var("KURO_KOKORO_DIR") {
        let path = PathBuf::from(path.trim());
        if path.is_dir() {
            roots.push(path);
        }
    }
    if let Some(dir) = kuro_core::kokoro_dir() {
        roots.push(dir);
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        roots.push(local.join("Kuro").join("models").join("kokoro"));
    }
    roots.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("bench")
            .join("models")
            .join("kokoro"),
    );
    roots
}

fn find_onnx_in(dir: &Path) -> Option<PathBuf> {
    for name in ["model_quantized.onnx", "model.onnx", "kokoro-v1.0.onnx", "model_fp16.onnx"] {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn find_voices_dir(dir: &Path) -> Option<PathBuf> {
    let voices = dir.join("voices");
    if voices.is_dir() {
        return Some(voices);
    }
    None
}

pub fn find_assets() -> Option<(PathBuf, PathBuf)> {
    for dir in model_roots() {
        if !dir.is_dir() {
            continue;
        }
        let onnx = find_onnx_in(&dir)?;
        let voices = find_voices_dir(&dir)?;
        return Some((onnx, voices));
    }
    None
}

pub fn ready() -> bool {
    find_assets().is_some()
}

pub fn voice_spec() -> String {
    std::env::var("KURO_KOKORO_VOICE").unwrap_or_else(|_| "af_heart".into())
}

#[derive(Debug, Clone)]
pub(crate) struct BlendPart {
    pub name: String,
    pub weight: f32,
}

pub(crate) fn parse_blend(spec: &str) -> Vec<BlendPart> {
    let mut parts = Vec::new();
    for chunk in spec.split('+') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let (name, weight) = if let Some((n, w)) = chunk.split_once(':') {
            (n.trim().to_string(), w.trim().parse::<f32>().unwrap_or(1.0))
        } else {
            (chunk.to_string(), 1.0)
        };
        if !name.is_empty() {
            parts.push(BlendPart { name, weight });
        }
    }
    if parts.is_empty() {
        parts.push(BlendPart { name: "af_heart".into(), weight: 1.0 });
    }
    parts
}

fn load_voice_rows(path: &Path) -> Option<Vec<f32>> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.as_chunks::<4>().0 {
        out.push(f32::from_le_bytes(*chunk));
    }
    if out.len() % STYLE_DIM != 0 {
        return None;
    }
    Some(out)
}

fn blend_style(
    voices_dir: &Path,
    voices_cache: &mut HashMap<String, Vec<f32>>,
    phoneme_n: usize,
    spec: &str,
) -> Option<[f32; STYLE_DIM]> {
    let parts = parse_blend(spec);
    let row = phoneme_n.min(MAX_PHONEMES).min(509);
    let mut acc = [0.0f32; STYLE_DIM];
    let mut wsum = 0.0f32;
    for part in parts {
        if !voices_cache.contains_key(&part.name) {
            let path = voices_dir.join(format!("{}.bin", part.name));
            let loaded = load_voice_rows(&path)?;
            voices_cache.insert(part.name.clone(), loaded);
        }
        let data = voices_cache.get(&part.name)?;
        let rows = data.len() / STYLE_DIM;
        let r = row.min(rows.saturating_sub(1));
        let start = r * STYLE_DIM;
        for i in 0..STYLE_DIM {
            acc[i] += part.weight * data[start + i];
        }
        wsum += part.weight;
    }
    if wsum <= 0.0 {
        return None;
    }
    for x in &mut acc {
        *x /= wsum;
    }
    Some(acc)
}

/// Prefer espeak-ng IPA; else a lightweight letter→IPA approximation.
fn phonemize(text: &str) -> String {
    if let Some(ipa) = espeak_ipa(text) {
        return ipa;
    }
    simple_g2p(text)
}

static ESPEAK_BIN: OnceLock<Option<PathBuf>> = OnceLock::new();

fn detect_espeak() -> Option<PathBuf> {
    ESPEAK_BIN
        .get_or_init(|| {
            let mut candidates = Vec::new();
            if let Ok(p) = std::env::var("KURO_ESPEAK_EXE") {
                candidates.push(PathBuf::from(p.trim()));
            }
            candidates.push(PathBuf::from("espeak-ng"));
            candidates.push(PathBuf::from("espeak"));
            candidates.push(PathBuf::from(r"C:\Program Files\eSpeak NG\espeak-ng.exe"));
            candidates.push(PathBuf::from(r"C:\Program Files (x86)\eSpeak NG\espeak-ng.exe"));

            for bin in candidates {
                if let Ok(out) = silent_cmd(&bin).arg("--version").output()
                    && out.status.success()
                {
                    return Some(bin);
                }
            }
            None
        })
        .clone()
}

fn espeak_ipa(text: &str) -> Option<String> {
    let bin = detect_espeak()?;
    let out = silent_cmd(&bin).args(["--ipa", "-q", "-v", "en-us", text]).output().ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn simple_g2p(text: &str) -> String {
    // Minimal English→IPA-ish so Kokoro always gets non-empty tokens when espeak is absent.
    let mut out = String::new();
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let w = word.to_ascii_lowercase();
        match w.as_str() {
            "the" => out.push_str("ðə"),
            "a" | "an" => out.push('ə'),
            "to" => out.push_str("tu"),
            "and" => out.push_str("ænd"),
            "of" => out.push_str("əv"),
            "you" => out.push_str("ju"),
            "i" => out.push_str("aɪ"),
            "desk" => out.push_str("dɛsk"),
            "on" => out.push_str("ɑn"),
            "off" => out.push_str("ɔf"),
            "hello" => out.push_str("hɛloʊ"),
            "good" => out.push_str("ɡʊd"),
            "night" => out.push_str("naɪt"),
            "sir" => out.push_str("sɝ"),
            "welcome" => out.push_str("wɛlkəm"),
            "home" => out.push_str("hoʊm"),
            "alright" => out.push_str("ɔlraɪt"),
            "anytime" => out.push_str("ɛnitaɪm"),
            _ => {
                for ch in w.chars() {
                    if ch.is_ascii_alphabetic() || matches!(ch, '.' | ',' | '?' | '!' | ';' | ':') {
                        out.push(ch);
                    }
                }
            }
        }
    }
    if out.is_empty() {
        out.push('ə');
    }
    out
}

fn tokenize_ipa(ipa: &str) -> Vec<i64> {
    let mut tokens = Vec::new();
    for ch in ipa.chars() {
        if let Some(id) = kokoro_char_id(ch) {
            if id == 0 {
                continue; // reserve 0 for pad
            }
            tokens.push(id);
            if tokens.len() >= MAX_PHONEMES {
                break;
            }
        }
    }
    tokens
}

fn resample_linear(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
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

struct KokoroRuntime {
    session: ort::session::Session,
    voices_dir: PathBuf,
    voices_cache: HashMap<String, Vec<f32>>,
}

static SLOT: Mutex<Option<KokoroRuntime>> = Mutex::new(None);

fn ensure() -> Result<(), String> {
    let mut slot = SLOT.lock().map_err(|_| "kokoro lock".to_string())?;
    if slot.is_some() {
        return Ok(());
    }
    let (onnx, voices) = find_assets().ok_or_else(|| "kokoro assets missing".to_string())?;
    let threads = std::thread::available_parallelism().map(|n| n.get().clamp(2, 6)).unwrap_or(4);
    let mut session = kuro_core::ort_env::session_from_file_with_threads(&onnx, threads)?;
    // Warm up ONNX Runtime graph optimization and arena allocation at startup.
    let dummy_ids = Tensor::from_array(([1usize, 3usize], vec![0i64, 1, 0])).map_err(|e| e.to_string())?;
    let dummy_style = Tensor::from_array(([1usize, STYLE_DIM], vec![0.0f32; STYLE_DIM])).map_err(|e| e.to_string())?;
    let dummy_speed = Tensor::from_array(([1usize], vec![1.0f32])).map_err(|e| e.to_string())?;
    let _ = session
        .run(ort::inputs![
            "input_ids" => dummy_ids,
            "style" => dummy_style,
            "speed" => dummy_speed,
        ])
        .map_err(|e| format!("kokoro warmup: {e}"))?;
    let mut voices_cache = HashMap::new();
    let _ = blend_style(&voices, &mut voices_cache, 1, &voice_spec());
    let _ = detect_espeak();
    *slot = Some(KokoroRuntime { session, voices_dir: voices, voices_cache });
    Ok(())
}

pub fn prewarm() -> Result<(), String> {
    ensure()
}

/// Split the opening clause of a >40-char sentence so first-chunk audio starts quickly.
pub(crate) fn split_first_clause(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    if text.chars().count() <= 40 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let consider = |best: &mut Option<(usize, usize)>, start_b: usize, end_b: usize| {
        if best.is_none_or(|(prev, _)| start_b < prev) {
            *best = Some((start_b, end_b));
        }
    };
    for delim in [". ", "? ", "! ", "; ", ": ", " \u{2014} "] {
        if let Some(pos) = text.find(delim)
            && text[..pos].chars().next().is_some()
        {
            let end_b = pos + delim.trim_end().len();
            consider(&mut best, pos, end_b);
        }
    }
    let mut search_from = 0usize;
    while let Some(rel) = text[search_from..].find(", ") {
        let pos = search_from + rel;
        if text[..pos].chars().count() >= 20 {
            consider(&mut best, pos, pos + 1);
            break;
        }
        search_from = pos + 2;
    }
    let (_start_b, end_b) = best?;
    let head = text[..end_b].trim();
    let tail = text[end_b..].trim();
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    Some((head.to_string(), tail.to_string()))
}

pub fn synthesize(text: &str) -> Option<Vec<f32>> {
    let text = text.trim();
    if text.is_empty() || !ready() {
        return None;
    }
    ensure().ok()?;
    let ipa = phonemize(text);
    let phonemes = tokenize_ipa(&ipa);
    if phonemes.is_empty() {
        return None;
    }
    let mut ids = Vec::with_capacity(phonemes.len() + 2);
    ids.push(0);
    ids.extend_from_slice(&phonemes);
    ids.push(0);
    let n = ids.len();

    let mut slot = SLOT.lock().ok()?;
    let rt = slot.as_mut()?;
    let style = blend_style(&rt.voices_dir, &mut rt.voices_cache, phonemes.len(), &voice_spec())?;

    let input_ids = Tensor::from_array(([1usize, n], ids)).ok()?;
    let style_t = Tensor::from_array(([1usize, STYLE_DIM], style.to_vec())).ok()?;
    let speed: f32 = std::env::var("KURO_KOKORO_SPEED").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.88);
    let speed_t = Tensor::from_array(([1usize], vec![speed])).ok()?;

    let outputs = rt
        .session
        .run(ort::inputs![
            "input_ids" => input_ids,
            "style" => style_t,
            "speed" => speed_t,
        ])
        .ok()?;
    let (_name, value) = outputs
        .iter()
        .find(|(n, _)| {
            let n: &str = n;
            n == "waveform" || n == "audio"
        })
        .or_else(|| outputs.iter().next())?;
    let (_shape, data) = value.try_extract_tensor::<f32>().ok()?;
    let pcm: Vec<f32> = data.to_vec();
    if pcm.is_empty() {
        return None;
    }
    Some(resample_linear(&pcm, KOKORO_HZ, PLAYBACK_HZ))
}

pub fn shutdown() {
    if let Ok(mut slot) = SLOT.lock() {
        *slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_parses_weights() {
        let parts = parse_blend("af_heart:0.65+af_bella:0.35");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].name, "af_heart");
        assert!((parts[0].weight - 0.65).abs() < f32::EPSILON);
    }

    #[test]
    fn first_clause_splits_long_sentences() {
        assert_eq!(split_first_clause("Short sentence."), None);
        let long = "First, let me check your smart plug status, and then I will report back.";
        let (head, tail) = split_first_clause(long).expect("should split on second comma");
        assert_eq!(head, "First, let me check your smart plug status,");
        assert_eq!(tail, "and then I will report back.");

        let colon = "Checking the status now: the desk plug is currently offline on the LAN.";
        let (head, tail) = split_first_clause(colon).expect("should split on colon");
        assert_eq!(head, "Checking the status now:");
        assert_eq!(tail, "the desk plug is currently offline on the LAN.");
    }

    #[test]
    fn synthesizes_when_model_present() {
        if !ready() {
            eprintln!("kokoro model absent; skip");
            return;
        }
        let init_start = std::time::Instant::now();
        prewarm().expect("kokoro prewarm");
        let init_ms = init_start.elapsed().as_secs_f64() * 1000.0;

        let start = std::time::Instant::now();
        let pcm = synthesize("Desk on.").expect("kokoro synth");
        let first_ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!("kokoro_init_warmup_ms={init_ms:.2} kokoro_first_audio_ms={first_ms:.2}");
        assert!(pcm.len() > 1000, "pcm too short: {}", pcm.len());

        let start = std::time::Instant::now();
        let pcm2 = synthesize("Alright.").expect("kokoro synth 2");
        let second_ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!("kokoro_second_uncached_ms={second_ms:.2} samples={}", pcm2.len());
    }
}
