use chrono::{DateTime, Local};
use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

const MANIFEST_TOML: &str = include_str!("feature.toml");

/// Same formatting as `kuro_router::phrases::clock_reply` for bridge parity.
pub fn format_clock_reply(local: DateTime<Local>) -> String {
    local.format("It's %I:%M %p.").to_string().trim_start_matches('0').replace(" 0", " ")
}

#[derive(Debug)]
pub struct TimeFeature {
    manifest: FeatureManifest,
}

impl TimeFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("time feature.toml");
        Self { manifest }
    }
}

impl Default for TimeFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for TimeFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, ctx: &FeatureContext<'_>, _slots: &Value) -> Result<Outcome, FeatureError> {
        let local = ctx.clock.now_utc().with_timezone(&Local);
        let speech = format_clock_reply(local);
        Ok(Outcome::say(speech))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
