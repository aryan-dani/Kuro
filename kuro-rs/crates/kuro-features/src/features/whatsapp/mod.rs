mod control;

use std::sync::Arc;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use control::{FakeWhatsappControl, NoopWhatsappControl, WhatsappControl};

const MANIFEST_TOML: &str = include_str!("feature.toml");

pub struct WhatsappFeature {
    manifest: FeatureManifest,
    control: Arc<dyn WhatsappControl>,
}

impl WhatsappFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("whatsapp feature.toml");
        Self { manifest, control: Arc::new(NoopWhatsappControl) }
    }

    pub fn with_control(control: Arc<dyn WhatsappControl>) -> Self {
        let mut feature = Self::loaded();
        feature.control = control;
        feature
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn slot_str<'a>(slots: &'a Value, key: &str) -> &'a str {
        slots.get(key).and_then(|v| v.as_str()).unwrap_or("").trim()
    }

    fn map_error(&self, raw: &str) -> String {
        let low = raw.to_lowercase();
        if low.contains("don't know who") {
            return self.reply("unknown_contact", "I don't know who to message.");
        }
        if low.contains("don't know how") {
            return self.reply("unknown_source", "I don't know how to send that.");
        }
        if low.contains("nothing to send") {
            return self.reply("empty_payload", "There is nothing to send.");
        }
        if low.contains("clipboard is empty") {
            return self.reply("empty_clipboard", "The clipboard is empty. Copy text or an image first.");
        }
        if low.contains("could not copy a link") {
            return self.reply("browser_link_failed", "I could not copy a link from the browser.");
        }
        if raw.is_empty() {
            return self.reply("unknown_contact", "I don't know who to message.");
        }
        raw.to_string()
    }
}

impl Default for WhatsappFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for WhatsappFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let contact = Self::slot_str(slots, "contact");
        let source = Self::slot_str(slots, "source");
        let text = Self::slot_str(slots, "text");
        if contact.is_empty() {
            return Ok(Outcome::say(self.reply("unknown_contact", "I don't know who to message.")));
        }
        match self.control.send(contact, source, text) {
            Ok(speech) => Ok(Outcome::say(speech)),
            Err(raw) => Ok(Outcome::say(self.map_error(&raw))),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
