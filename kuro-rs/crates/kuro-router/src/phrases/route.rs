use super::handle::try_handle;
use super::matchers::{is_hub_desk, is_music_only, matches};
use super::normalize::normalize;

#[derive(Clone, Debug)]
pub struct RouteResult {
    pub reply: String,
    pub skip_tts: bool,
    pub force_fast: bool,
    pub start_ollama: bool,
}

pub fn route_command(
    text: &str,
    live: bool,
    hub_handled: Option<Result<Option<serde_json::Value>, ()>>,
    llm_mode: &str,
    ollama_up: bool,
) -> RouteResult {
    if !live {
        return RouteResult { reply: String::new(), skip_tts: true, force_fast: true, start_ollama: false };
    }
    if let Some(local) = try_handle(text) {
        return RouteResult {
            reply: local.reply,
            skip_tts: local.skip_tts,
            force_fast: local.force_fast_tts,
            start_ollama: false,
        };
    }
    if let Some(result) = hub_handled {
        if !live {
            return RouteResult { reply: String::new(), skip_tts: true, force_fast: true, start_ollama: false };
        }
        match result {
            Err(()) => {
                let t = normalize(text);
                if is_music_only(&t)
                    && let Some(local) = try_handle(text)
                {
                    return RouteResult {
                        reply: local.reply,
                        skip_tts: local.skip_tts,
                        force_fast: local.force_fast_tts,
                        start_ollama: false,
                    };
                }
                if is_hub_desk(&t) {
                    return RouteResult {
                        reply: "I can't reach the hub.".into(),
                        skip_tts: false,
                        force_fast: false,
                        start_ollama: false,
                    };
                }
            }
            Ok(Some(value)) if value.get("duplicate").and_then(|v| v.as_bool()).unwrap_or(false) => {
                return RouteResult { reply: String::new(), skip_tts: true, force_fast: false, start_ollama: false };
            }
            Ok(Some(value)) if value.get("handled").and_then(|v| v.as_bool()).unwrap_or(false) => {
                if let Some(actions) = value.get("client_actions").and_then(|v| v.as_array()) {
                    for action in actions {
                        kuro_tools::music::apply_action(action);
                    }
                }
                return RouteResult {
                    reply: value.get("reply").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    skip_tts: false,
                    force_fast: false,
                    start_ollama: false,
                };
            }
            _ => {}
        }
    }
    if let Some(local) = try_handle(text) {
        return RouteResult {
            reply: local.reply,
            skip_tts: local.skip_tts,
            force_fast: local.force_fast_tts,
            start_ollama: false,
        };
    }
    // Actuation requests that missed the tool phrase: honest miss, never chat waffle.
    if kuro_tools::honesty::wants_tool_actuation(text) {
        return RouteResult {
            reply: kuro_tools::honesty::honest_tool_miss(text),
            skip_tts: false,
            force_fast: true,
            start_ollama: false,
        };
    }
    if llm_mode == "off" {
        return RouteResult {
            reply: "I don't have a phrase for that yet. Desk, news, music, meet, WhatsApp, and workflows work offline."
                .into(),
            skip_tts: false,
            force_fast: true,
            start_ollama: false,
        };
    }
    if !ollama_up {
        return RouteResult {
            reply: "Neural engine is off. Please wait while I start it.".into(),
            skip_tts: false,
            force_fast: true,
            start_ollama: true,
        };
    }
    RouteResult { reply: String::new(), skip_tts: false, force_fast: false, start_ollama: true }
}

pub fn prefer_user_text(live: &str, final_text: &str) -> String {
    kuro_core::prefer_transcript(live, final_text, matches(live), matches(final_text))
}
