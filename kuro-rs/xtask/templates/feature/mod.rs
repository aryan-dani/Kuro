use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct {{type_name}} {
    manifest: FeatureManifest,
}

impl {{type_name}} {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("{{id}} feature.toml");
        Self { manifest }
    }
}

impl Feature for {{type_name}} {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, _ctx: &FeatureContext<'_>, _slots: &Value) -> Result<Outcome, FeatureError> {
        let speech = self.manifest.replies.get("ok").map(|s| s.as_str()).unwrap_or("Done.");
        Ok(Outcome::say(speech))
    }
}

impl Default for {{type_name}} {
    fn default() -> Self {
        Self::loaded()
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;