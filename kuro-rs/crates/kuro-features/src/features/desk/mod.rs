mod control;

use std::sync::Arc;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use control::{DeskCall, DeskControl, FakeDeskControl, NoopDeskControl};

const MANIFEST_TOML: &str = include_str!("feature.toml");

pub struct DeskFeature {
    manifest: FeatureManifest,
    control: Arc<dyn DeskControl>,
}

impl DeskFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("desk feature.toml");
        Self { manifest, control: Arc::new(NoopDeskControl) }
    }

    pub fn with_control(control: Arc<dyn DeskControl>) -> Self {
        let mut feature = Self::loaded();
        feature.control = control;
        feature
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn scene(slots: &Value) -> Option<&str> {
        slots.get("scene").and_then(|v| v.as_str()).or_else(|| slots.get("intent").and_then(|v| v.as_str()))
    }

    fn reply_key(scene: &str) -> (&'static str, &'static str) {
        match scene {
            "power_on" | "desk.power_on" => ("power_on", "Desk on."),
            "power_off" | "desk.power_off" => ("power_off", "Desk off."),
            "good_night" | "desk.good_night" => ("good_night", "Desk off. Good night, sir."),
            _ => ("unreachable", "I couldn't reach the plug."),
        }
    }

    fn normalize_scene(raw: &str) -> Option<&'static str> {
        match raw {
            "power_on" | "desk.power_on" => Some("power_on"),
            "power_off" | "desk.power_off" => Some("power_off"),
            "good_night" | "desk.good_night" => Some("good_night"),
            _ => None,
        }
    }
}

impl Default for DeskFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for DeskFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let raw = Self::scene(slots).ok_or_else(|| {
            FeatureError::invalid_input("desk feature needs slots.scene (power_on, power_off, good_night)")
        })?;
        let scene = Self::normalize_scene(raw)
            .ok_or_else(|| FeatureError::invalid_input(format!("unknown desk scene: {raw}")))?;
        match self.control.apply_scene(scene) {
            Ok(()) => {
                let (key, fallback) = Self::reply_key(scene);
                Ok(Outcome::say(self.reply(key, fallback)))
            }
            Err(raw) => {
                let low = raw.to_lowercase();
                if ["can't reach", "unable", "error", "missing", "offline"].iter().any(|k| low.contains(k)) {
                    if raw.len() < 160 {
                        Ok(Outcome::say(raw))
                    } else {
                        Ok(Outcome::say(self.reply("unreachable", "I couldn't reach the plug.")))
                    }
                } else if raw.is_empty() {
                    Ok(Outcome::say(self.reply("unreachable", "I couldn't reach the plug.")))
                } else {
                    let (key, fallback) = Self::reply_key(scene);
                    Ok(Outcome::say(self.reply(key, fallback)))
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
