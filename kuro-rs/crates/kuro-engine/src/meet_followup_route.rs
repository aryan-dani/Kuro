//! Hybrid Meet follow-up: soft understanding + join_meet tool, without phrase lists.

use serde_json::json;

use crate::turn_agent::AgentRoute;

/// When the phrase router missed, still join a remembered Meet from soft signals.
pub(crate) fn try_soft_meet_followup(text: &str, skip_tts: bool) -> Option<AgentRoute> {
    let found = kuro_tools::meet_followup_match(text)?;
    let contact = found.get("contact").cloned().unwrap_or_default();
    let raw = kuro_tools::execute_tool("join_meet", &json!({ "contact": contact }));
    let speech = speakable(&raw);
    if speech.is_empty() {
        return None;
    }
    Some(AgentRoute::Reply(speech, skip_tts))
}

fn speakable(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.to_lowercase().starts_with("success:") {
        text = text.split_once(':').map(|(_, rest)| rest.trim().to_string()).unwrap_or(text);
    }
    text
}
