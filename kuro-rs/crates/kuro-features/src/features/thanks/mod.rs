use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct ThanksFeature {
    manifest: FeatureManifest,
}

impl ThanksFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("thanks feature.toml");
        Self { manifest }
    }
}

impl Default for ThanksFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for ThanksFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, _slots: &Value) -> Result<Outcome, FeatureError> {
        let text = self.manifest.replies.get("ok").map(|s| s.as_str()).unwrap_or("Anytime.");
        Ok(Outcome::say(text))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
