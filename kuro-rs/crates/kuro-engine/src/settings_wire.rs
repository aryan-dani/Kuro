//! Apply `kuro_config::Settings` into engine runtime surfaces.

use kuro_config::Settings;
use kuro_core::EngineConfig;
use kuro_llm::AgentConfig;

/// Load settings once and push values tools/IPC still read from the environment.
pub fn load_and_apply_runtime_settings() -> Settings {
    let settings = Settings::load().unwrap_or_default();
    apply_runtime_env(&settings);
    settings
}

pub fn agent_config_from_settings(settings: &Settings) -> AgentConfig {
    AgentConfig { enabled: settings.agent.enabled, max_steps: settings.agent.max_steps }
}

pub fn engine_config_from_settings(settings: &Settings) -> EngineConfig {
    EngineConfig {
        llm_mode: settings.llm_mode.clone(),
        stt_backend: settings.stt_backend.clone(),
        tts_engine: settings.tts_engine.clone(),
        xtts_language: std::env::var("XTTS_LANGUAGE").unwrap_or_else(|_| "en".into()),
        paused: false,
        hud_show_text: matches!(
            std::env::var("KURO_HUD_SHOW_TEXT").unwrap_or_default().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        plug_id: std::env::var("TUYA_DEVICE_ID").unwrap_or_default(),
    }
}

fn apply_runtime_env(settings: &Settings) {
    // Safety: process startup only; bridges Settings into crates that still read env.
    unsafe {
        if settings.say_execute {
            std::env::set_var("KURO_SAY_EXECUTE", "1");
        }
        if let Some(url) = &settings.hub_url {
            std::env::set_var("KURO_HUB_URL", url);
        }
        if let Some(token) = &settings.hub_token {
            std::env::set_var("KURO_HUB_TOKEN", token);
        }
        if let Some(url) = &settings.n8n_base_url {
            std::env::set_var("KURO_N8N_BASE_URL", url);
        }
        if let Some(token) = &settings.n8n_token {
            std::env::set_var("KURO_N8N_TOKEN", token);
        }
        if let Some(dir) = &settings.data_dir {
            std::env::set_var("KURO_DATA_DIR", dir);
        }
        std::env::set_var("KURO_LLM_MODE", &settings.llm_mode);
        std::env::set_var("KURO_STT_BACKEND", &settings.stt_backend);
        std::env::set_var("KURO_TTS_ENGINE", &settings.tts_engine);
        std::env::set_var("KURO_API_PORT", settings.api_port.to_string());
        // Product default: play TTS unless the launcher explicitly disabled it.
        match std::env::var("KURO_TTS_PLAY") {
            Ok(v) if matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "off" | "no") => {}
            Ok(v) if !v.trim().is_empty() => {}
            _ => std::env::set_var("KURO_TTS_PLAY", "1"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_config_copies_modes() {
        let settings = Settings {
            llm_mode: "off".into(),
            stt_backend: "cpu".into(),
            tts_engine: "kokoro".into(),
            ..Settings::default()
        };
        let cfg = engine_config_from_settings(&settings);
        assert_eq!(cfg.llm_mode, "off");
        assert_eq!(cfg.stt_backend, "cpu");
        assert_eq!(cfg.tts_engine, "kokoro");
    }
}
