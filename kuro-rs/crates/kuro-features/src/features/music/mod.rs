mod control;

use std::sync::Arc;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use control::{FakeMusicControl, MusicCall, MusicControl, NoopMusicControl};

const MANIFEST_TOML: &str = include_str!("feature.toml");

pub struct MusicFeature {
    manifest: FeatureManifest,
    control: Arc<dyn MusicControl>,
}

impl MusicFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("music feature.toml");
        Self { manifest, control: Arc::new(NoopMusicControl) }
    }

    pub fn with_control(control: Arc<dyn MusicControl>) -> Self {
        let mut feature = Self::loaded();
        feature.control = control;
        feature
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn op(slots: &Value) -> Option<&str> {
        slots.get("op").and_then(|v| v.as_str())
    }

    fn normalize_op(raw: &str) -> Option<&'static str> {
        match raw {
            "pause" | "music.pause" => Some("pause"),
            "resume" | "music.resume" => Some("resume"),
            "stop" | "music.stop" => Some("stop"),
            _ => None,
        }
    }
}

impl Default for MusicFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for MusicFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let raw = Self::op(slots)
            .ok_or_else(|| FeatureError::invalid_input("music feature needs slots.op (pause, resume, stop)"))?;
        let op =
            Self::normalize_op(raw).ok_or_else(|| FeatureError::invalid_input(format!("unknown music op: {raw}")))?;

        let result = match op {
            "pause" => self.control.pause(),
            "resume" => self.control.resume(),
            "stop" => self.control.stop(),
            _ => unreachable!("normalized op"),
        };

        match result {
            Ok(speech) => Ok(Outcome::say(speech)),
            Err(raw) if raw.is_empty() => Ok(Outcome::say(self.reply("unavailable", "Music player is unavailable."))),
            Err(raw) => {
                let low = raw.to_lowercase();
                if low.contains("unavailable") {
                    Ok(Outcome::say(self.reply("unavailable", "Music player is unavailable.")))
                } else {
                    Ok(Outcome::say(raw))
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
