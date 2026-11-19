mod ritual;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::features::calendar::CalendarFeature;
use crate::features::recall::RecallFeature;
use crate::features::weather::WeatherFeature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use ritual::{RitualFile, RitualStep, parse_ritual};

const MANIFEST_TOML: &str = include_str!("feature.toml");
const RITUAL_TOML: &str = include_str!("ritual.toml");

#[derive(Debug)]
pub struct MorningBriefFeature {
    manifest: FeatureManifest,
    ritual: RitualFile,
    weather: WeatherFeature,
    calendar: CalendarFeature,
    recall: RecallFeature,
}

impl MorningBriefFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("morning_brief feature.toml");
        let ritual = parse_ritual(RITUAL_TOML).expect("morning_brief ritual.toml");
        Self {
            manifest,
            ritual,
            weather: WeatherFeature::default(),
            calendar: CalendarFeature::default(),
            recall: RecallFeature::default(),
        }
    }

    pub fn record_recall(&self, question: &str) {
        self.recall.record_turn(None, Some(question));
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn intent(slots: &Value) -> Option<&str> {
        slots.get("intent").and_then(|v| v.as_str()).or_else(|| slots.get("action").and_then(|v| v.as_str()))
    }

    fn include_recall(slots: &Value) -> bool {
        slots.get("include_recall").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    fn feature_disabled(slots: &Value) -> bool {
        slots.get("enabled").and_then(|v| v.as_bool()) == Some(false)
    }

    fn step_slots(parent: &Value, intent: &str) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("intent".into(), Value::String(intent.to_string()));
        for key in ["weather_json", "ics_data", "ics_path"] {
            if let Some(v) = parent.get(key) {
                obj.insert(key.into(), v.clone());
            }
        }
        Value::Object(obj)
    }

    fn run_step(&self, step: &RitualStep, ctx: &FeatureContext<'_>, slots: &Value) -> Option<String> {
        let child = Self::step_slots(slots, &step.intent);
        let outcome = match step.feature.as_str() {
            "weather" => self.weather.execute(ctx, &child),
            "calendar" => self.calendar.execute(ctx, &child),
            "recall" => self.recall.execute(ctx, &child),
            _ => return None,
        };
        match outcome {
            Ok(out) => {
                let text = out.speech.as_str().trim();
                if text.is_empty() || Self::is_empty_recall(text) || Self::is_not_configured(text) {
                    None
                } else {
                    Some(text.to_string())
                }
            }
            Err(_) => None,
        }
    }

    fn is_empty_recall(text: &str) -> bool {
        text.starts_with("I do not have a recent question") || text.starts_with("I do not have anything to repeat")
    }

    fn is_not_configured(text: &str) -> bool {
        text.contains("not configured")
    }
}

impl Default for MorningBriefFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for MorningBriefFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let intent = Self::intent(slots)
            .ok_or_else(|| FeatureError::invalid_input("morning_brief needs slots.intent (morning_brief.run)"))?;
        if intent != "morning_brief.run" {
            return Err(FeatureError::invalid_input(format!("unknown morning_brief intent: {intent}")));
        }
        if Self::feature_disabled(slots) {
            return Ok(Outcome::say(self.reply("disabled", "Morning brief is turned off in your profile.")));
        }
        Ok(self.compose_brief(ctx, slots))
    }
}

impl MorningBriefFeature {
    fn compose_brief(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Outcome {
        let mut parts: Vec<String> = vec![self.reply("greeting", "Good morning.")];
        let recall_on = Self::include_recall(slots);
        for step in &self.ritual.steps {
            if step.feature == "recall" && step.optional && !recall_on {
                continue;
            }
            if let Some(line) = self.run_step(step, ctx, slots) {
                parts.push(line);
            }
        }
        if parts.len() == 1 {
            parts.push(self.reply("empty", "Your morning brief is empty. Set weather or calendar in your profile."));
        }
        Outcome::say(parts.join(" "))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
