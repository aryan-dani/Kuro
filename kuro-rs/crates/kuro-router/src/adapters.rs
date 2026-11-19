//! Live feature control adapters that call `kuro-tools` at the router boundary.

use std::sync::Arc;

use kuro_features::features::desk::{DeskControl, DeskFeature};
use kuro_features::features::meet::{MeetControl, MeetFeature};
use kuro_features::features::music::{MusicControl, MusicFeature};
use kuro_features::features::whatsapp::{WhatsappControl, WhatsappFeature};
use kuro_features::{FeatureContext, SystemClock};
use serde_json::{Value, json};

#[derive(Debug, Default, Clone, Copy)]
pub struct ToolsDeskControl;

impl DeskControl for ToolsDeskControl {
    fn apply_scene(&self, scene: &str) -> Result<(), String> {
        let raw = kuro_tools::execute_tool("control_smart_plug", &json!({"scene": scene}));
        map_plug_actuation(&raw)
    }
}

fn map_plug_actuation(raw: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err(String::new());
    }
    let low = raw.to_lowercase();
    if low.starts_with("success") {
        return Ok(());
    }
    if ["can't reach", "unable", "error", "missing", "offline"].iter().any(|k| low.contains(k)) {
        return Err(raw.to_string());
    }
    Ok(())
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ToolsMusicControl;

impl MusicControl for ToolsMusicControl {
    fn pause(&self) -> Result<String, String> {
        Ok(kuro_tools::music::pause_song())
    }

    fn resume(&self) -> Result<String, String> {
        Ok(kuro_tools::music::resume_song())
    }

    fn stop(&self) -> Result<String, String> {
        Ok(kuro_tools::music::stop_song())
    }
}

pub(crate) fn desk_scene_reply(scene: &str) -> String {
    let feature = DeskFeature::with_control(Arc::new(ToolsDeskControl));
    execute_feature_speech(&feature, &json!({ "scene": scene }))
}

pub(crate) fn music_op_reply(op: &str) -> String {
    let intent_name = match op {
        "pause" => "PauseMusic",
        "resume" => "ResumeMusic",
        "stop" => "StopMusic",
        _ => return String::new(),
    };
    let feature = MusicFeature::with_control(Arc::new(ToolsMusicControl));
    let live = execute_feature_speech(&feature, &json!({ "op": op }));
    merge_music_reply(intent_name, live)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ToolsMeetControl;

impl MeetControl for ToolsMeetControl {
    fn create_meet(&self, contact: &str) -> Result<String, String> {
        let raw = kuro_tools::execute_tool("make_meet", &json!({ "contact": contact }));
        map_tool_speech(&raw)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ToolsWhatsappControl;

impl WhatsappControl for ToolsWhatsappControl {
    fn send(&self, contact: &str, source: &str, text: &str) -> Result<String, String> {
        let raw =
            kuro_tools::execute_tool("send_whatsapp", &json!({ "contact": contact, "source": source, "text": text }));
        map_tool_speech(&raw)
    }
}

pub(crate) fn meet_reply(t: &str) -> String {
    let args = kuro_tools::match_registered_phrase(t)
        .filter(|(tool, _)| tool.name == "make_meet")
        .map(|(_, args)| args)
        .unwrap_or_else(|| json!({ "contact": "" }));
    let feature = MeetFeature::with_control(Arc::new(ToolsMeetControl));
    let live = execute_feature_speech(&feature, &args);
    merge_optional_reply("MakeMeet", live, &args)
}

pub(crate) fn whatsapp_reply(t: &str) -> String {
    let Some((_, args)) = kuro_tools::match_registered_phrase(t).filter(|(tool, _)| tool.name == "send_whatsapp")
    else {
        return "I don't know who to message.".into();
    };
    let feature = WhatsappFeature::with_control(Arc::new(ToolsWhatsappControl));
    let live = execute_feature_speech(&feature, &args);
    merge_optional_reply("SendWhatsapp", live, &args)
}

fn map_tool_speech(raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(String::new());
    }
    let low = text.to_lowercase();
    if low.starts_with("success:") {
        return Ok(speakable(text));
    }
    if low.starts_with("error:") {
        return Err(text.to_string());
    }
    Err(text.to_string())
}

fn speakable(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.to_lowercase().starts_with("success:") {
        text = text.split_once(':').map(|(_, rest)| rest.trim().to_string()).unwrap_or(text);
    }
    if text.is_empty() { "Done, sir.".into() } else { text }
}

fn merge_optional_reply(intent_name: &str, live: String, slots: &Value) -> String {
    match kuro_features::spoken_for_intent_with_slots(intent_name, slots) {
        Some(feature) if feature == live => feature,
        _ => live,
    }
}

fn merge_music_reply(intent_name: &str, live: String) -> String {
    match kuro_features::spoken_for_intent(intent_name) {
        Some(noop) if noop == live => noop,
        Some(noop) if live == "Music player is unavailable." => noop,
        _ => live,
    }
}

fn execute_feature_speech(feature: &dyn kuro_features::Feature, slots: &Value) -> String {
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    match feature.execute(&ctx, slots) {
        Ok(outcome) => outcome.speech.as_str().to_string(),
        Err(err) => err.detail().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, OnceLock};

    fn record_calls() -> &'static Mutex<Vec<(String, Value)>> {
        static CALLS: OnceLock<Mutex<Vec<(String, Value)>>> = OnceLock::new();
        CALLS.get_or_init(|| Mutex::new(Vec::new()))
    }

    #[test]
    fn tools_desk_control_maps_success() {
        record_calls().lock().unwrap().clear();
        kuro_tools::set_executor(Some(Box::new(|name, args| {
            record_calls().lock().unwrap().push((name.to_string(), args.clone()));
            "Success: ok".into()
        })));
        let ctrl = ToolsDeskControl;
        assert!(ctrl.apply_scene("power_on").is_ok());
        let recorded = record_calls().lock().unwrap().clone();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "control_smart_plug");
        assert_eq!(recorded[0].1["scene"], "power_on");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn tools_desk_control_surfaces_offline() {
        kuro_tools::set_executor(Some(Box::new(|_, _| "unable to reach device".into())));
        let err = ToolsDeskControl.apply_scene("power_off").unwrap_err();
        assert!(err.contains("unable"));
        kuro_tools::set_executor(None);
    }

    #[test]
    fn desk_scene_reply_uses_manifest_on_success() {
        kuro_tools::set_executor(Some(Box::new(|_, _| "Success: ok".into())));
        let reply = desk_scene_reply("power_on");
        assert_eq!(reply, "Desk on.");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn tools_meet_control_maps_success() {
        let recorded: Arc<Mutex<Vec<(String, Value)>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);
        kuro_tools::set_executor(Some(Box::new(move |name, args| {
            sink.lock().unwrap().push((name.to_string(), args.clone()));
            "Success: The Meet is open.".into()
        })));
        let ctrl = ToolsMeetControl;
        assert_eq!(ctrl.create_meet("").unwrap(), "The Meet is open.");
        let calls = recorded.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "make_meet");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn meet_reply_uses_manifest_on_corpus_fake() {
        kuro_tools::set_executor(Some(Box::new(|name, args| {
            if name == "make_meet" {
                if args.get("contact").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
                    "Success: The Meet is open.".into()
                } else {
                    let contact = args["contact"].as_str().unwrap_or("contact");
                    format!("Success: Sent the Meet link to {contact}.")
                }
            } else {
                "Success: ok".into()
            }
        })));
        assert_eq!(meet_reply("make a google meet"), "The Meet is open.");
        assert_eq!(meet_reply("send a google meet link to mom"), "Sent the Meet link to mom.");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn whatsapp_reply_uses_manifest_on_corpus_fake() {
        kuro_tools::set_executor(Some(Box::new(|name, _| {
            if name == "send_whatsapp" { "Success: Sent.".into() } else { "Success: ok".into() }
        })));
        let reply = whatsapp_reply("send this link to siya on whatsapp");
        assert_eq!(reply, "Sent.");
        kuro_tools::set_executor(None);
    }
}
