use std::path::PathBuf;

use crate::settings::Settings;

fn non_empty(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub(crate) fn apply_env(settings: &mut Settings) {
    if let Ok(v) = std::env::var("KURO_LLM_MODE") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            settings.llm_mode = trimmed.to_string();
        }
    }
    if let Ok(v) = std::env::var("KURO_STT_BACKEND") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            settings.stt_backend = trimmed.to_string();
        }
    }
    if let Ok(v) = std::env::var("KURO_TTS_ENGINE") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            settings.tts_engine = trimmed.to_string();
        }
    }
    if let Ok(v) = std::env::var("KURO_API_PORT")
        && let Ok(port) = v.trim().parse::<u16>()
        && (1..=65535).contains(&port)
    {
        settings.api_port = port;
    }
    if let Ok(v) = std::env::var("KURO_HUB_URL") {
        settings.hub_url = non_empty(Some(v));
    }
    if let Ok(v) = std::env::var("KURO_HUB_TOKEN") {
        settings.hub_token = non_empty(Some(v));
    }
    if let Ok(v) = std::env::var("KURO_N8N_BASE_URL") {
        settings.n8n_base_url = non_empty(Some(v));
    }
    if let Ok(v) = std::env::var("KURO_N8N_TOKEN") {
        settings.n8n_token = non_empty(Some(v));
    }
    if std::env::var("KURO_SAY_EXECUTE").ok().as_deref() == Some("1") {
        settings.say_execute = true;
    }
    if let Ok(v) = std::env::var("KURO_DATA_DIR") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            settings.data_dir = Some(PathBuf::from(trimmed));
        }
    }
    if matches!(std::env::var("KURO_WAKE_WORD").ok().as_deref(), Some("1") | Some("true") | Some("yes") | Some("on")) {
        settings.wake_word_enabled = true;
    }
    if let Ok(v) = std::env::var("KURO_WAKE_WORD_MODEL") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            settings.wake_word_model = Some(PathBuf::from(trimmed));
        }
    }
    if matches!(std::env::var("KURO_LISTENER").ok().as_deref(), Some("1") | Some("true") | Some("yes") | Some("on")) {
        settings.listener_enabled = true;
    }
    if matches!(
        std::env::var("KURO_AGENT_ENABLED").ok().as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    ) {
        settings.agent.enabled = true;
    }
    if let Ok(v) = std::env::var("KURO_AGENT_MAX_STEPS")
        && let Ok(max) = v.trim().parse::<u32>()
        && max > 0
    {
        settings.agent.max_steps = max;
    }
}
