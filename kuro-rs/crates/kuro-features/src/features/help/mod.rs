use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct HelpFeature {
    manifest: FeatureManifest,
}

impl HelpFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("help feature.toml");
        Self { manifest }
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn intent(slots: &Value) -> Option<&str> {
        slots.get("intent").and_then(|v| v.as_str()).or_else(|| slots.get("action").and_then(|v| v.as_str()))
    }
}

impl Default for HelpFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for HelpFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        if Self::intent(slots) == Some("help.what_can_i_say") {
            return Ok(self.speak_discover(slots));
        }
        let text = self.manifest.replies.get("ok").map(|s| s.as_str()).unwrap_or(
            "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. Say run hello workflow to test n8n.",
        );
        Ok(Outcome::say(text))
    }
}

impl HelpFeature {
    fn speak_discover(&self, slots: &Value) -> Outcome {
        let entries = registry_entries(slots);
        if entries.is_empty() {
            return Outcome::say(self.reply("discover_empty", "No features are registered yet."));
        }
        let header = self.reply("discover_header", "Here is what you can say.");
        let line_template = self.reply("discover_line", "{title}: say phrases routed to {id}.");
        let mut lines: Vec<String> = Vec::with_capacity(entries.len());
        for (id, title) in entries {
            lines.push(line_template.replace("{title}", title.as_str()).replace("{id}", id.as_str()));
        }
        Outcome::say(format!("{header} {}", lines.join(" ")))
    }
}

fn registry_entries(slots: &Value) -> Vec<(String, String)> {
    if let Some(snapshot) = slots.get("registry_snapshot").and_then(|v| v.as_array()) {
        let mut out = Vec::new();
        for item in snapshot {
            let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or(id).to_string();
            out.push((id.to_string(), title));
        }
        if !out.is_empty() {
            return out;
        }
    }
    slots
        .get("feature_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|id| (id.to_string(), id.to_string())).collect())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
