use serde_json::Value;

use crate::context::{FeatureContext, SystemClock};
use crate::error::FeatureError;
use crate::registry::FeatureRegistry;

/// Optional shim: run a registered feature and return only the spoken string.
pub fn execute_spoken_registry(
    registry: &FeatureRegistry,
    id: &str,
    ctx: &FeatureContext<'_>,
    slots: &Value,
) -> Result<String, FeatureError> {
    registry.execute_spoken(id, ctx, slots)
}

/// Default registry + system clock. Intended for unit tests until the router bridge lands.
pub fn execute_spoken(id: &str) -> Result<String, FeatureError> {
    let registry = FeatureRegistry::with_features();
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    registry.execute_spoken(id, &ctx, &Value::Null)
}

/// Maps semantic `Intent` variant names to registered feature ids.
pub fn feature_id_for_intent(intent_name: &str) -> Option<&'static str> {
    match intent_name {
        "Cancel" => Some("cancel"),
        "Thanks" => Some("thanks"),
        "Help" => Some("help"),
        "Identity" => Some("identity"),
        "Time" => Some("time"),
        "PowerOn" | "PowerOff" | "GoodNight" => Some("desk"),
        "PauseMusic" | "ResumeMusic" | "StopMusic" => Some("music"),
        "MakeMeet" => Some("meet"),
        "SendWhatsapp" => Some("whatsapp"),
        _ => None,
    }
}

fn desk_slots_for_intent(intent_name: &str) -> Option<Value> {
    let scene = match intent_name {
        "PowerOn" => "power_on",
        "PowerOff" => "power_off",
        "GoodNight" => "good_night",
        _ => return None,
    };
    Some(serde_json::json!({ "scene": scene }))
}

fn music_slots_for_intent(intent_name: &str) -> Option<Value> {
    let op = match intent_name {
        "PauseMusic" => "pause",
        "ResumeMusic" => "resume",
        "StopMusic" => "stop",
        _ => return None,
    };
    Some(serde_json::json!({ "op": op }))
}

fn execute_spoken_with_slots(id: &str, slots: &Value) -> Option<String> {
    let registry = FeatureRegistry::with_features();
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    registry.execute_spoken(id, &ctx, slots).ok()
}

/// Spoken reply for a router intent name, or None if unbridged or execution failed.
pub fn spoken_for_intent(intent_name: &str) -> Option<String> {
    let id = feature_id_for_intent(intent_name)?;
    if let Some(slots) = desk_slots_for_intent(intent_name) {
        return execute_spoken_with_slots(id, &slots);
    }
    if let Some(slots) = music_slots_for_intent(intent_name) {
        return execute_spoken_with_slots(id, &slots);
    }
    execute_spoken(id).ok()
}

/// Spoken reply when router slots (tool args) are already resolved.
pub fn spoken_for_intent_with_slots(intent_name: &str, slots: &Value) -> Option<String> {
    let id = feature_id_for_intent(intent_name)?;
    execute_spoken_with_slots(id, slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn spoken_for_intent_maps_known_names() {
        assert_eq!(spoken_for_intent("Cancel").as_deref(), Some("Alright."));
        assert_eq!(spoken_for_intent("PowerOn").as_deref(), Some("Desk on."));
        assert_eq!(spoken_for_intent("PowerOff").as_deref(), Some("Desk off."));
        assert_eq!(spoken_for_intent("GoodNight").as_deref(), Some("Desk off. Good night, sir."));
        assert_eq!(spoken_for_intent("PauseMusic").as_deref(), Some("Nothing is playing."));
        assert_eq!(spoken_for_intent("ResumeMusic").as_deref(), Some("Nothing to resume."));
        assert_eq!(spoken_for_intent("StopMusic").as_deref(), Some("Music stopped."));
        assert!(spoken_for_intent("Date").is_none());
        assert!(spoken_for_intent("unknown").is_none());
    }

    #[test]
    fn meet_and_whatsapp_bridge_match_corpus_noop() {
        let open = spoken_for_intent_with_slots("MakeMeet", &serde_json::json!({"contact": ""}));
        assert_eq!(open.as_deref(), Some("The Meet is open."));
        let sent = spoken_for_intent_with_slots("MakeMeet", &serde_json::json!({"contact": "mom"}));
        assert_eq!(sent.as_deref(), Some("Sent the Meet link to mom."));
        let wa = spoken_for_intent_with_slots(
            "SendWhatsapp",
            &serde_json::json!({"contact": "siya", "source": "browser_url", "text": ""}),
        );
        assert_eq!(wa.as_deref(), Some("Sent."));
    }

    #[test]
    fn spoken_replies_match_corpus_classes() {
        for (id, expected) in [
            ("cancel", "Alright."),
            ("thanks", "Anytime."),
            (
                "help",
                "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. Say run hello workflow to test n8n.",
            ),
            ("identity", "Kuro. Local assistant. Phrases first, model last."),
        ] {
            let got = execute_spoken(id).expect(id);
            assert_eq!(got, expected, "feature {id}");
        }
    }

    #[test]
    fn time_reply_matches_router_prefix() {
        let got = execute_spoken("time").expect("time");
        assert!(got.starts_with("It's "), "got: {got}");
    }

    #[test]
    fn format_clock_sample() {
        let local = Local.with_ymd_and_hms(2026, 6, 15, 14, 30, 0).unwrap();
        let got = crate::features::time::format_clock_reply(local);
        assert_eq!(got, "It's 2:30 PM.");
    }
}
