//! Kokoro / Piper speech with clip cache, Windows fallback, and sentence streaming.

mod kokoro;
mod piper;
mod sapi;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Once;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TtsKind {
    Auto,
    Kokoro,
    Piper,
    Xtts,
    Sapi,
}

pub fn kokoro_models_present() -> bool {
    kokoro::ready()
}

pub fn select_kind(requested: &str) -> TtsKind {
    match requested.trim().to_ascii_lowercase().as_str() {
        "kokoro" => TtsKind::Kokoro,
        "piper" => TtsKind::Piper,
        "xtts" => TtsKind::Xtts,
        "pyttsx3" => TtsKind::Sapi,
        "auto" => {
            if kokoro::ready() {
                TtsKind::Kokoro
            } else if piper::default_model().is_some() {
                TtsKind::Piper
            } else {
                TtsKind::Sapi
            }
        }
        _ => TtsKind::Piper,
    }
}

fn active_kind() -> TtsKind {
    select_kind(&std::env::var("KURO_TTS_ENGINE").unwrap_or_else(|_| "auto".into()))
}

pub fn active_engine_name(requested: &str) -> &'static str {
    match select_kind(requested) {
        TtsKind::Kokoro | TtsKind::Auto => {
            if kokoro::ready() {
                "kokoro"
            } else if piper::default_model().is_some() {
                "piper"
            } else {
                "sapi"
            }
        }
        TtsKind::Piper | TtsKind::Xtts => {
            if piper::default_model().is_some() {
                "piper"
            } else {
                "sapi"
            }
        }
        TtsKind::Sapi => "sapi",
    }
}

pub fn prewarm() -> Result<(), String> {
    if matches!(active_kind(), TtsKind::Kokoro | TtsKind::Auto) && kokoro::ready() {
        kokoro::prewarm()?;
    }
    Ok(())
}

const BOOT_PHRASES: &[&str] = &[
    "Desk on.",
    "Desk off.",
    "Desk off. Good night, sir.",
    "Welcome home, sir.",
    "Already on it, sir.",
    "Alright.",
    "Anytime.",
    "I can't reach the hub.",
    "I couldn't reach the plug.",
];

static PREBUILD: Once = Once::new();

pub fn cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Kuro")
        .join("tts_cache")
}

fn cache_file(text: &str) -> PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    // v3: piper idle window widened; ignore older truncated clips.
    cache_dir().join(format!("v3-{:016x}.f32", hasher.finish()))
}

fn load_disk_pcm(text: &str) -> Option<Vec<f32>> {
    let path = cache_file(text);
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 4 || bytes.len() % 4 != 0 {
        return None;
    }
    let mut samples = Vec::with_capacity(bytes.len() / 4);
    let (chunks, _rest) = bytes.as_chunks::<4>();
    for chunk in chunks {
        samples.push(f32::from_le_bytes(*chunk));
    }
    Some(samples)
}

fn store_disk_pcm(text: &str, pcm: &[f32]) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut bytes = Vec::with_capacity(pcm.len() * 4);
    for sample in pcm {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    let _ = std::fs::write(cache_file(text), bytes);
}

pub fn shutdown_worker() {
    kokoro::shutdown();
    piper::shutdown();
    kuro_core::ort_env::shutdown();
}

pub fn prebuild_phrase_cache() {
    PREBUILD.call_once(|| {
        let _ = std::fs::create_dir_all(cache_dir());
        for phrase in BOOT_PHRASES {
            if load_disk_pcm(phrase).is_some() {
                continue;
            }
            let pcm = synthesize(phrase);
            if !pcm.is_empty() {
                store_disk_pcm(phrase, &pcm);
            }
        }
    });
}

pub struct Speaker {
    cache: HashMap<String, Vec<f32>>,
    synth_calls: u32,
}

impl Default for Speaker {
    fn default() -> Self {
        let mut speaker = Self { cache: HashMap::new(), synth_calls: 0 };
        for phrase in BOOT_PHRASES {
            if let Some(pcm) = load_disk_pcm(phrase) {
                speaker.cache.insert((*phrase).to_string(), pcm);
            }
        }
        speaker
    }
}

impl Speaker {
    pub fn synth_calls(&self) -> u32 {
        self.synth_calls
    }

    pub fn cached(&self, text: &str) -> bool {
        self.cache.contains_key(text.trim())
    }

    /// Synthesize or reuse a clip, then call `on_audio_start` only when the first samples
    /// are appended to the output stream and this turn is still live.
    pub fn speak<L, F>(&mut self, text: &str, live: L, mut on_audio_start: F) -> bool
    where
        L: Fn() -> bool,
        F: FnMut(),
    {
        let key = text.trim();
        if key.is_empty() || !live() {
            return false;
        }
        if self.cache.contains_key(key) {
            if !live() {
                return false;
            }
            let pcm = self.cache.get(key).cloned().unwrap_or_default();
            return self.play_cached(key, &pcm, &live, &mut on_audio_start);
        }

        let sentences = split_sentences(key);
        if sentences.len() <= 1 {
            let pcm = if let Some(disk) = load_disk_pcm(key) {
                disk
            } else {
                let pcm = synthesize(key);
                if !pcm.is_empty() {
                    store_disk_pcm(key, &pcm);
                }
                pcm
            };
            if pcm.is_empty() {
                return false;
            }
            self.synth_calls += 1;
            self.cache.insert(key.to_string(), pcm.clone());
            return self.play_cached(key, &pcm, &live, &mut on_audio_start);
        }

        self.synth_calls += 1;
        let mut assembled = Vec::new();
        let mut prefetch: Option<std::thread::JoinHandle<Vec<f32>>> = None;
        let mut started_audio = false;
        for (idx, sent) in sentences.iter().enumerate() {
            if !live() {
                return false;
            }
            let pcm =
                if let Some(handle) = prefetch.take() { handle.join().unwrap_or_default() } else { synthesize(sent) };
            if idx + 1 < sentences.len() {
                let next = sentences[idx + 1].clone();
                prefetch = Some(std::thread::spawn(move || synthesize(&next)));
            }
            if pcm.is_empty() {
                continue;
            }
            assembled.extend_from_slice(&pcm);
            if std::env::var("KURO_TTS_PLAY").ok().as_deref() == Some("1") {
                let mut hook = || {
                    if !started_audio {
                        started_audio = true;
                        on_audio_start();
                    }
                };
                if play_pcm_live(&pcm, &live, &mut hook).is_err() {
                    return false;
                }
            } else if !started_audio {
                started_audio = true;
                on_audio_start();
            }
        }
        if assembled.is_empty() {
            return false;
        }
        store_disk_pcm(key, &assembled);
        self.cache.insert(key.to_string(), assembled);
        started_audio
    }

    fn play_cached<L, F>(&self, _key: &str, pcm: &[f32], live: &L, on_audio_start: &mut F) -> bool
    where
        L: Fn() -> bool,
        F: FnMut(),
    {
        if std::env::var("KURO_TTS_PLAY").ok().as_deref() == Some("1") {
            return play_pcm_live(pcm, live, on_audio_start).is_ok();
        }
        on_audio_start();
        true
    }
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if matches!(chars[i], '.' | '?' | '!') && i + 1 < chars.len() && chars[i + 1] == ' ' {
            let sentence: String = chars[start..=i].iter().collect();
            let trimmed = sentence.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            start = i + 2;
            i += 2;
            continue;
        }
        i += 1;
    }
    let tail: String = chars[start..].iter().collect();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    if out.is_empty() && !text.trim().is_empty() {
        out.push(text.trim().to_string());
    }
    if let Some(first) = out.first().cloned()
        && let Some((head, tail)) = kokoro::split_first_clause(&first)
    {
        out[0] = head;
        out.insert(1, tail);
    }
    out
}

pub fn synthesize(text: &str) -> Vec<f32> {
    match active_kind() {
        TtsKind::Sapi => sapi::synthesize(text).unwrap_or_default(),
        TtsKind::Kokoro => {
            if let Some(pcm) = kokoro::synthesize(text) {
                return pcm;
            }
            if let Some(pcm) = piper::synthesize(text) {
                return pcm;
            }
            sapi::synthesize(text).unwrap_or_default()
        }
        TtsKind::Auto => {
            if let Some(pcm) = kokoro::synthesize(text) {
                return pcm;
            }
            if let Some(pcm) = piper::synthesize(text) {
                return pcm;
            }
            sapi::synthesize(text).unwrap_or_default()
        }
        TtsKind::Piper | TtsKind::Xtts => {
            if let Some(pcm) = piper::synthesize(text) {
                return pcm;
            }
            sapi::synthesize(text).unwrap_or_default()
        }
    }
}

fn play_pcm_live<L, F>(pcm: &[f32], live: &L, on_audio_start: &mut F) -> Result<(), String>
where
    L: Fn() -> bool,
    F: FnMut(),
{
    if pcm.is_empty() {
        return Ok(());
    }
    if !live() {
        return Ok(());
    }
    let (stream, handle) = rodio::OutputStream::try_default().map_err(|err| err.to_string())?;
    let sink = rodio::Sink::try_new(&handle).map_err(|err| err.to_string())?;
    let source = rodio::buffer::SamplesBuffer::new(1, 22_050, pcm.to_vec());
    if !live() {
        return Ok(());
    }
    sink.append(source);
    on_audio_start();
    while !sink.empty() {
        if !live() {
            sink.stop();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    drop(stream);
    Ok(())
}

pub fn play_pcm(pcm: &[f32]) -> Result<(), String> {
    play_pcm_live(pcm, &|| true, &mut || {})
}

pub fn speak_sapi(text: &str) -> Result<(), String> {
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Speak({text:?})"
    );
    let mut cmd = std::process::Command::new("powershell");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let status = cmd.args(["-NoProfile", "-Command", &script]).status().map_err(|err| err.to_string())?;
    if status.success() { Ok(()) } else { Err("Windows speech failed.".into()) }
}
