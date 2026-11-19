mod parse;

use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use parse::{WeatherSnapshot, parse_weather_json};

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct WeatherFeature {
    manifest: FeatureManifest,
}

impl WeatherFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("weather feature.toml");
        Self { manifest }
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn intent(slots: &Value) -> Option<&str> {
        slots.get("intent").and_then(|v| v.as_str()).or_else(|| slots.get("action").and_then(|v| v.as_str()))
    }

    fn weather_body<'a>(ctx: &FeatureContext<'a>, slots: &'a Value) -> Option<&'a str> {
        slots.get("weather_json").and_then(|v| v.as_str()).or(ctx.weather_json)
    }
}

impl Default for WeatherFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for WeatherFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let intent = Self::intent(slots)
            .ok_or_else(|| FeatureError::invalid_input("weather feature needs slots.intent (weather.now)"))?;
        match intent {
            "weather.now" => self.speak_now(ctx, slots),
            other => Err(FeatureError::invalid_input(format!("unknown weather intent: {other}"))),
        }
    }
}

impl WeatherFeature {
    fn speak_now(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let Some(body) = Self::weather_body(ctx, slots) else {
            return Ok(Outcome::say(self.reply("not_configured", "Weather is not configured.")));
        };
        let snapshot = parse_weather_json(body).map_err(|err| FeatureError::invalid_input(err.message))?;
        let template = self.reply("now_ok", "It is {temp} degrees.");
        let speech = template.replace("{temp}", &format_temp(snapshot.celsius));
        Ok(Outcome::say(speech))
    }
}

fn format_temp(celsius: f64) -> String {
    if celsius.fract().abs() < f64::EPSILON { format!("{}", celsius.round() as i64) } else { format!("{celsius:.1}") }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
