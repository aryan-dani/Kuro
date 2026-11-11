//! Whisper backend selection, prompt bias, and live-vs-final choice.

mod worker;

use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SttBackend {
    Cuda,
    Npu,
    Cpu,
}

impl SttBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cuda => "cuda",
            Self::Npu => "npu",
            Self::Cpu => "cpu",
        }
    }
}

pub fn select_backend(requested: &str, on_ac: bool) -> SttBackend {
    match requested.trim().to_ascii_lowercase().as_str() {
        "cuda" => SttBackend::Cuda,
        "npu" => SttBackend::Npu,
        "cpu" => SttBackend::Cpu,
        _ => {
            if on_ac {
                SttBackend::Cuda
            } else {
                SttBackend::Npu
            }
        }
    }
}

pub fn on_ac_line(ac_line_status: u8) -> bool {
    ac_line_status == 1
}

pub fn query_on_ac() -> bool {
    unsafe {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut status = std::mem::zeroed::<SYSTEM_POWER_STATUS>();
        if GetSystemPowerStatus(&mut status) == 0 {
            return true;
        }
        on_ac_line(status.ACLineStatus)
    }
}

pub fn beam_size(partial: bool) -> u32 {
    if partial {
        return 1;
    }
    std::env::var("KURO_WHISPER_BEAM")
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .filter(|n: &u32| *n > 0)
        .unwrap_or(1)
}

const PROMPT_BASE: &str = "Wake up daddy's home. Good night sir. Pause the song. Resume the song. Stop the music. What's the news today. Turn on the lights. Turn off the lights. Switch the lights off. Monitor off. Monitor on. Desk off. Desk on. Power on. Power off. Any urgent mail. Inbox check. Deadlines today. Hello Kuro. Send the Google Meet link to Mom. Make a Google Meet.";

pub fn initial_prompt(aliases: &[String], contacts: &[String]) -> String {
    let mut text = PROMPT_BASE.to_string();
    if !aliases.is_empty() {
        text.push(' ');
        text.push_str(&aliases.join(". "));
        text.push('.');
    }
    if !contacts.is_empty() {
        let joined = contacts.iter().take(12).cloned().collect::<Vec<_>>().join(", ");
        text.push_str(&format!(" Send this link to {joined} on WhatsApp."));
    }
    text
}

pub fn model_dir(backend: SttBackend, repo: &Path) -> PathBuf {
    if let Ok(override_dir) = std::env::var("KURO_OV_WHISPER_DIR")
        && backend == SttBackend::Npu
        && !override_dir.trim().is_empty()
    {
        return PathBuf::from(override_dir.trim());
    }
    match backend {
        SttBackend::Npu => repo.join("bench").join("models").join("whisper-small.en-int8"),
        SttBackend::Cuda | SttBackend::Cpu => {
            let name = std::env::var("KURO_WHISPER_MODEL").unwrap_or_else(|_| "small.en".into());
            repo.join("bench").join("models").join(name)
        }
    }
}

pub fn model_present(path: &Path) -> bool {
    path.is_dir() || path.is_file()
}

/// Prefer the live partial when final is empty, or when only the live text is a known phrase.
pub fn choose_transcript(live: &str, final_text: &str, live_match: bool, final_match: bool) -> String {
    kuro_core::prefer_transcript(live, final_text, live_match, final_match)
}

pub fn ready_for_heavy(on_ac: bool, legion: Option<&Value>, now_secs: f64) -> bool {
    if !on_ac {
        return false;
    }
    let Some(status) = legion else {
        return false;
    };
    let updated = status.get("updated").and_then(|v| v.as_f64()).unwrap_or(-1.0);
    let age = now_secs - updated;
    if !(0.0..=45.0).contains(&age) {
        return false;
    }
    if status.get("ac_online").and_then(|v| v.as_bool()) != Some(true) {
        return false;
    }
    if status.get("smart_fan_mode").and_then(|v| v.as_i64()) != Some(3) {
        return false;
    }
    status.get("ac_fit").and_then(|v| v.as_i64()) != Some(0)
}

/// Transcribe mono f32 audio with in-process whisper-rs (GGML).
pub fn transcribe_pcm(pcm: &[f32], sample_rate: u32, partial: bool, prompt: &str) -> Result<String, String> {
    let backend = select_backend(&std::env::var("KURO_STT_BACKEND").unwrap_or_else(|_| "auto".into()), query_on_ac());
    let device = if backend == SttBackend::Cuda { "cuda" } else { "cpu" };
    worker::transcribe(pcm, sample_rate, partial, prompt, device)
}

pub fn prewarm() -> Result<(), String> {
    let backend = select_backend(&std::env::var("KURO_STT_BACKEND").unwrap_or_else(|_| "auto".into()), query_on_ac());
    let device = if backend == SttBackend::Cuda { "cuda" } else { "cpu" };
    worker::prewarm(device)
}

pub fn shutdown_worker() {
    worker::shutdown();
}
