mod turn_log;

use std::sync::Mutex;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use turn_log::TurnLog;

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct RecallFeature {
    manifest: FeatureManifest,
    log: Mutex<TurnLog>,
}

impl RecallFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("recall feature.toml");
        Self { manifest, log: Mutex::new(TurnLog::default()) }
    }

    /// Seed log state (tests and future turn-machine hook).
    pub fn record_turn(&self, spoken: Option<&str>, question: Option<&str>) {
        let mut log = self.log.lock().expect("recall log");
        if let Some(text) = spoken {
            log.record_spoken(text);
        }
        if let Some(text) = question {
            log.record_question(text);
        }
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn intent(slots: &Value) -> Option<&str> {
        slots.get("intent").and_then(|v| v.as_str()).or_else(|| slots.get("action").and_then(|v| v.as_str()))
    }
}

impl Default for RecallFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for RecallFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let intent = Self::intent(slots).ok_or_else(|| {
            FeatureError::invalid_input("recall feature needs slots.intent (recall.repeat, recall.last_question)")
        })?;
        let log = self.log.lock().expect("recall log");
        match intent {
            "recall.repeat" => {
                let Some(text) = log.last_spoken() else {
                    return Ok(Outcome::say(self.reply("empty_repeat", "I do not have anything to repeat yet.")));
                };
                let template = self.reply("repeat_ok", "{text}");
                Ok(Outcome::say(template.replace("{text}", text)))
            }
            "recall.last_question" => {
                let Some(text) = log.last_question() else {
                    return Ok(Outcome::say(self.reply("empty_question", "I do not have a recent question saved.")));
                };
                let template = self.reply("question_ok", "You asked: {text}");
                Ok(Outcome::say(template.replace("{text}", text)))
            }
            other => Err(FeatureError::invalid_input(format!("unknown recall intent: {other}"))),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
