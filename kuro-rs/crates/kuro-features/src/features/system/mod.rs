mod control;

#[cfg(windows)]
mod win;

use std::sync::Arc;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use control::{FakeSystemControl, NoopSystemControl, SystemCall, SystemControl};

const MANIFEST_TOML: &str = include_str!("feature.toml");

pub struct SystemFeature {
    manifest: FeatureManifest,
    control: Arc<dyn SystemControl>,
}

impl SystemFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("system feature.toml");
        Self { manifest, control: Arc::new(NoopSystemControl) }
    }

    pub fn with_control(control: Arc<dyn SystemControl>) -> Self {
        let mut feature = Self::loaded();
        feature.control = control;
        feature
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn intent(slots: &Value) -> Option<&str> {
        slots.get("intent").and_then(|v| v.as_str()).or_else(|| slots.get("action").and_then(|v| v.as_str()))
    }

    fn parse_level(slots: &Value) -> Option<u8> {
        if let Some(n) = slots.get("level").and_then(|v| v.as_u64()) {
            return u8::try_from(n).ok();
        }
        slots.get("level").and_then(|v| v.as_str())?.parse().ok()
    }
}

impl Default for SystemFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for SystemFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let intent = Self::intent(slots).ok_or_else(|| {
            FeatureError::invalid_input(
                "system feature needs slots.intent (system.volume_set, system.mute, system.lock)",
            )
        })?;
        match intent {
            "system.volume_set" => {
                let level = Self::parse_level(slots).unwrap_or(50);
                self.control.volume_set(level);
                let template = self.reply("volume_ok", "Volume set to {level} percent.");
                Ok(Outcome::say(template.replace("{level}", &level.to_string())))
            }
            "system.mute" => {
                self.control.mute();
                Ok(Outcome::say(self.reply("mute_ok", "Muted.")))
            }
            "system.lock" => {
                self.control.lock();
                Ok(Outcome::say(self.reply("lock_ok", "Locking now.")))
            }
            other => Err(FeatureError::invalid_input(format!("unknown system intent: {other}"))),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
