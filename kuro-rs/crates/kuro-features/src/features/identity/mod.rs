use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct IdentityFeature {
    manifest: FeatureManifest,
}

impl IdentityFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("identity feature.toml");
        Self { manifest }
    }
}

impl Default for IdentityFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for IdentityFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, _slots: &Value) -> Result<Outcome, FeatureError> {
        let text = self
            .manifest
            .replies
            .get("ok")
            .map(|s| s.as_str())
            .unwrap_or("Kuro. Local assistant. Phrases first, model last.");
        Ok(Outcome::say(text))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
