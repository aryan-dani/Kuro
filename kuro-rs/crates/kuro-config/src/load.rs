use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::settings::Settings;

#[derive(Debug)]
pub enum ConfigLoadError {
    ProfileRead(std::io::Error),
    ProfileParse(toml::de::Error),
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileRead(e) => write!(f, "read profile: {e}"),
            Self::ProfileParse(e) => write!(f, "parse profile: {e}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

pub fn repo_root() -> PathBuf {
    kuro_core::repo_root_hint()
}

pub fn example_profile_path() -> PathBuf {
    repo_root().join("profiles").join("example").join("config.toml")
}

pub fn user_profile_path() -> PathBuf {
    if let Ok(dir) = std::env::var("KURO_DATA_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("profile").join("config.toml");
        }
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Kuro")
        .join("profile")
        .join("config.toml")
}

pub(crate) fn load_settings() -> Result<Settings, ConfigLoadError> {
    let mut settings = Settings::default();
    // defaults < example profile < user profile < env
    let example = example_profile_path();
    if example.is_file() {
        apply_profile_file(&mut settings, &example)?;
    }
    let user = user_profile_path();
    if user.is_file() {
        apply_profile_file(&mut settings, &user)?;
    }
    apply_env(&mut settings);
    Ok(settings)
}

#[derive(Default, Deserialize)]
struct HubFile {
    url: Option<String>,
    token: Option<String>,
}

#[derive(Default, Deserialize)]
struct N8nFile {
    base_url: Option<String>,
    token: Option<String>,
}

#[derive(Default, Deserialize)]
struct WakeWordFile {
    enabled: Option<bool>,
    model: Option<String>,
}

#[derive(Default, Deserialize)]
struct AgentFile {
    enabled: Option<bool>,
    max_steps: Option<u32>,
}

#[derive(Default, Deserialize)]
struct ListenerFile {
    enabled: Option<bool>,
}

#[derive(Default, Deserialize)]
struct ProfileFile {
    llm_mode: Option<String>,
    stt_backend: Option<String>,
    tts_engine: Option<String>,
    api_port: Option<u16>,
    say_execute: Option<bool>,
    data_dir: Option<String>,
    hub: Option<HubFile>,
    n8n: Option<N8nFile>,
    wake_word: Option<WakeWordFile>,
    listener: Option<ListenerFile>,
    agent: Option<AgentFile>,
}

fn apply_profile_file(settings: &mut Settings, path: &Path) -> Result<(), ConfigLoadError> {
    let raw = std::fs::read_to_string(path).map_err(ConfigLoadError::ProfileRead)?;
    let file: ProfileFile = toml::from_str(&raw).map_err(ConfigLoadError::ProfileParse)?;
    if let Some(v) = file.llm_mode {
        settings.llm_mode = v;
    }
    if let Some(v) = file.stt_backend {
        settings.stt_backend = v;
    }
    if let Some(v) = file.tts_engine {
        settings.tts_engine = v;
    }
    if let Some(v) = file.api_port
        && (1..=65535).contains(&v)
    {
        settings.api_port = v;
    }
    if let Some(v) = file.say_execute {
        settings.say_execute = v;
    }
    if let Some(v) = file.data_dir {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            settings.data_dir = Some(PathBuf::from(trimmed));
        }
    }
    if let Some(hub) = file.hub {
        settings.hub_url = non_empty(hub.url);
        settings.hub_token = non_empty(hub.token);
    }
    if let Some(n8n) = file.n8n {
        settings.n8n_base_url = non_empty(n8n.base_url);
        settings.n8n_token = non_empty(n8n.token);
    }
    if let Some(wake) = file.wake_word {
        if let Some(enabled) = wake.enabled {
            settings.wake_word_enabled = enabled;
        }
        if let Some(model) = wake.model {
            let trimmed = model.trim();
            if !trimmed.is_empty() {
                settings.wake_word_model = Some(PathBuf::from(trimmed));
            }
        }
    }
    if let Some(listener) = file.listener
        && let Some(enabled) = listener.enabled
    {
        settings.listener_enabled = enabled;
    }
    if let Some(agent) = file.agent {
        if let Some(enabled) = agent.enabled {
            settings.agent.enabled = enabled;
        }
        if let Some(max_steps) = agent.max_steps.filter(|&n| n > 0) {
            settings.agent.max_steps = max_steps;
        }
    }
    Ok(())
}

fn non_empty(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

use crate::env::apply_env;

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
