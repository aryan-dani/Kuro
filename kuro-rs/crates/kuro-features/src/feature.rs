use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::manifest::FeatureManifest;
use crate::outcome::Outcome;

/// Extension point for a voice feature (ADR 0003 skeleton).
///
/// Full ADR surface adds `parse`, `health`, and `on_event`; those land with the
/// router registry wiring in later phases.
pub trait Feature: Send + Sync {
    fn id(&self) -> &str;
    fn manifest(&self) -> &FeatureManifest;
    fn execute(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError>;
}
