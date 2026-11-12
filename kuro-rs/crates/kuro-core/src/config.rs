use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub llm_mode: String,
    pub stt_backend: String,
    pub tts_engine: String,
    pub xtts_language: String,
    pub paused: bool,
    pub hud_show_text: bool,
    pub plug_id: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            llm_mode: std::env::var("KURO_LLM_MODE").unwrap_or_else(|_| "fallback".into()),
            stt_backend: std::env::var("KURO_STT_BACKEND").unwrap_or_else(|_| "auto".into()),
            tts_engine: std::env::var("KURO_TTS_ENGINE").unwrap_or_else(|_| "auto".into()),
            xtts_language: std::env::var("XTTS_LANGUAGE").unwrap_or_else(|_| "en".into()),
            paused: false,
            hud_show_text: env_flag("KURO_HUD_SHOW_TEXT"),
            plug_id: std::env::var("TUYA_DEVICE_ID").unwrap_or_default(),
        }
    }
}

fn env_flag(name: &str) -> bool {
    matches!(std::env::var(name).unwrap_or_default().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfigError;

const LLM: &[&str] = &["off", "fallback", "always"];
const STT: &[&str] = &["auto", "cuda", "npu", "cpu"];
const TTS: &[&str] = &["auto", "kokoro", "piper", "xtts", "pyttsx3"];

pub fn apply_config(cfg: &mut EngineConfig, payload: &Value) -> Result<(), ConfigError> {
    let obj = payload.as_object().ok_or(ConfigError)?;
    const ALLOWED: &[&str] = &["llm_mode", "stt_backend", "tts_engine", "xtts_language", "paused", "hud_show_text"];
    if obj.keys().any(|k| !ALLOWED.contains(&k.as_str())) {
        return Err(ConfigError);
    }
    if let Some(value) = obj.get("llm_mode") {
        let value = value.as_str().unwrap_or("").trim().to_ascii_lowercase();
        if !LLM.contains(&value.as_str()) {
            return Err(ConfigError);
        }
        cfg.llm_mode = value;
    }
    if let Some(value) = obj.get("stt_backend") {
        let value = value.as_str().unwrap_or("").trim().to_ascii_lowercase();
        if !STT.contains(&value.as_str()) {
            return Err(ConfigError);
        }
        cfg.stt_backend = value;
    }
    if let Some(value) = obj.get("tts_engine") {
        let value = value.as_str().unwrap_or("").trim().to_ascii_lowercase();
        if !TTS.contains(&value.as_str()) {
            return Err(ConfigError);
        }
        cfg.tts_engine = value;
    }
    if let Some(value) = obj.get("xtts_language") {
        let value = value.as_str().unwrap_or("").trim().to_ascii_lowercase();
        if !value.chars().all(|c| c.is_ascii_alphabetic()) || !(2..=8).contains(&value.len()) {
            return Err(ConfigError);
        }
        cfg.xtts_language = value;
    }
    if let Some(value) = obj.get("paused") {
        cfg.paused = value.as_bool().ok_or(ConfigError)?;
    }
    if let Some(value) = obj.get("hud_show_text") {
        cfg.hud_show_text = value.as_bool().ok_or(ConfigError)?;
    }
    Ok(())
}

pub fn public_config(cfg: &EngineConfig) -> Value {
    json!({
        "llm_mode": cfg.llm_mode,
        "stt_backend": cfg.stt_backend,
        "tts_engine": cfg.tts_engine,
        "xtts_language": cfg.xtts_language,
        "paused": cfg.paused,
        "hud_show_text": cfg.hud_show_text,
        "plug_id_masked": mask_id(&cfg.plug_id),
    })
}

pub fn mask_number(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return "masked".into();
    }
    let keep = &digits[digits.len() - 2..];
    format!("{}{keep}", "x".repeat(digits.len() - 2))
}

pub fn mask_id(raw: &str) -> String {
    let text = raw.trim();
    if text.len() <= 4 {
        return if text.is_empty() { String::new() } else { "xxxx".into() };
    }
    let chars: Vec<char> = text.chars().collect();
    let mid = "x".repeat(chars.len() - 4);
    format!("{}{mid}{}", chars[..2].iter().collect::<String>(), chars[chars.len() - 2..].iter().collect::<String>())
}
