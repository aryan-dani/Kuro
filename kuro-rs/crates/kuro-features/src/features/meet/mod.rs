mod control;

use std::sync::Arc;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use control::{FakeMeetControl, MeetControl, NoopMeetControl};

const MANIFEST_TOML: &str = include_str!("feature.toml");

pub struct MeetFeature {
    manifest: FeatureManifest,
    control: Arc<dyn MeetControl>,
}

impl MeetFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("meet feature.toml");
        Self { manifest, control: Arc::new(NoopMeetControl) }
    }

    pub fn with_control(control: Arc<dyn MeetControl>) -> Self {
        let mut feature = Self::loaded();
        feature.control = control;
        feature
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn contact(slots: &Value) -> &str {
        slots.get("contact").and_then(|v| v.as_str()).unwrap_or("").trim()
    }

    fn is_join(slots: &Value) -> bool {
        slots.get("action").and_then(|v| v.as_str()).unwrap_or("") == "join"
    }

    fn map_error(&self, raw: &str) -> String {
        let low = raw.to_lowercase();
        if low.contains("don't know that contact") {
            return self.reply("unknown_contact", "I don't know that contact.");
        }
        if low.contains("not signed in") {
            return self.reply("not_signed_in", "Google Meet is not signed in.");
        }
        if low.contains("already opening") {
            return self.reply("busy", "I'm already opening a Meet.");
        }
        if low.contains("don't have a meet link") {
            return self.reply("no_link", "I don't have a Meet link for them.");
        }
        if low.contains("couldn't find microsoft edge") {
            return self.reply("no_edge", "I couldn't find Microsoft Edge.");
        }
        if low.contains("couldn't create") {
            return self.reply("create_failed", "I couldn't create that Google Meet.");
        }
        if low.contains("couldn't send") {
            return self.reply("send_failed", "I couldn't send that Meet link.");
        }
        if raw.is_empty() {
            return self.reply("create_failed", "I couldn't create that Google Meet.");
        }
        raw.to_string()
    }
}

impl Default for MeetFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for MeetFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let contact = Self::contact(slots);
        let result =
            if Self::is_join(slots) { self.control.join_meet(contact) } else { self.control.create_meet(contact) };
        match result {
            Ok(speech) => Ok(Outcome::say(speech)),
            Err(raw) => Ok(Outcome::say(self.map_error(&raw))),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
