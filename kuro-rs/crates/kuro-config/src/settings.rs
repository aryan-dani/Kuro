use std::path::PathBuf;

/// LLM agent loop (phase 26). Off by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSettings {
    pub enabled: bool,
    pub max_steps: u32,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self { enabled: false, max_steps: 3 }
    }
}

/// Engine-facing knobs loaded from defaults, profile TOML, then env.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub llm_mode: String,
    pub stt_backend: String,
    pub tts_engine: String,
    pub api_port: u16,
    pub hub_url: Option<String>,
    pub hub_token: Option<String>,
    pub n8n_base_url: Option<String>,
    pub n8n_token: Option<String>,
    pub say_execute: bool,
    pub data_dir: Option<PathBuf>,
    /// Opt-in always-on wake word (phase 25). Default off; hold-to-talk stays primary.
    pub wake_word_enabled: bool,
    pub wake_word_model: Option<PathBuf>,
    /// Opt-in continuous listener loop (phase 20). Default off; PTT stays primary.
    pub listener_enabled: bool,
    pub agent: AgentSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm_mode: "fallback".into(),
            stt_backend: "auto".into(),
            tts_engine: "auto".into(),
            api_port: 8765,
            hub_url: None,
            hub_token: None,
            n8n_base_url: None,
            n8n_token: None,
            say_execute: false,
            data_dir: None,
            wake_word_enabled: false,
            wake_word_model: None,
            listener_enabled: false,
            agent: AgentSettings::default(),
        }
    }
}

impl Settings {
    /// Defaults, then example profile, then user profile, then env.
    pub fn load() -> Result<Self, super::ConfigLoadError> {
        super::load::load_settings()
    }
}
