//! Live `kuro-features` controls backed by `kuro-tools` (for the engine agent registry).

use std::sync::Arc;

use kuro_features::FeatureRegistry;
use kuro_features::features::meet::{MeetControl, MeetFeature};
use kuro_features::features::whatsapp::{WhatsappControl, WhatsappFeature};
use serde_json::json;

struct LiveWhatsappControl;

impl WhatsappControl for LiveWhatsappControl {
    fn send(&self, contact: &str, source: &str, text: &str) -> Result<String, String> {
        let src = if source.trim().is_empty() { "text" } else { source.trim() };
        let args = json!({ "contact": contact, "source": src, "text": text });
        let raw = kuro_tools::execute_tool("send_whatsapp", &args);
        if let Some(rest) = raw.strip_prefix("Success:") { Ok(rest.trim().to_string()) } else { Err(raw) }
    }
}

struct LiveMeetControl;

impl MeetControl for LiveMeetControl {
    fn create_meet(&self, contact: &str) -> Result<String, String> {
        let args = if contact.trim().is_empty() { json!({}) } else { json!({ "contact": contact }) };
        let raw = kuro_tools::execute_tool("make_meet", &args);
        if let Some(rest) = raw.strip_prefix("Success:") { Ok(rest.trim().to_string()) } else { Err(raw) }
    }

    fn join_meet(&self, contact: &str) -> Result<String, String> {
        let args = if contact.trim().is_empty() { json!({}) } else { json!({ "contact": contact }) };
        let raw = kuro_tools::execute_tool("join_meet", &args);
        if let Some(rest) = raw.strip_prefix("Success:") { Ok(rest.trim().to_string()) } else { Err(raw) }
    }
}

/// Build the engine's `FeatureRegistry` with live WhatsApp and Meet controls.
pub(crate) fn live_registry() -> FeatureRegistry {
    let mut registry = FeatureRegistry::with_features();
    registry.register_or_replace(Box::new(WhatsappFeature::with_control(Arc::new(LiveWhatsappControl))));
    registry.register_or_replace(Box::new(MeetFeature::with_control(Arc::new(LiveMeetControl))));
    registry
}
