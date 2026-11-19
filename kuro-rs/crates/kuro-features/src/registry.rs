use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::outcome::Outcome;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryError {
    pub message: String,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RegistryError {}

pub struct FeatureRegistry {
    entries: Vec<Box<dyn Feature>>,
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn register(&mut self, feature: Box<dyn Feature>) {
        self.entries.push(feature);
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|f| f.id()).collect()
    }

    pub fn get(&self, id: &str) -> Option<&dyn Feature> {
        self.entries.iter().find(|f| f.id() == id).map(|f| f.as_ref() as &dyn Feature)
    }

    pub fn execute(&self, id: &str, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let Some(feature) = self.get(id) else {
            return Err(FeatureError::invalid_input(format!("unknown feature id: {id}")));
        };
        feature.execute(ctx, slots)
    }

    /// Optional bridge: spoken string only (for tests and future router shim).
    pub fn execute_spoken(&self, id: &str, ctx: &FeatureContext<'_>, slots: &Value) -> Result<String, FeatureError> {
        self.execute(id, ctx, slots).map(|outcome| outcome.speech.as_str().to_string())
    }
}

impl FeatureRegistry {
    pub fn with_features() -> Self {
        let mut registry = Self::new();
        crate::features::register_all(&mut registry);
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::FakeClock;
    use chrono::TimeZone;

    fn sample_registry() -> FeatureRegistry {
        FeatureRegistry::with_features()
    }

    #[test]
    fn lists_first_slice_features() {
        let registry = sample_registry();
        for id in [
            "cancel",
            "thanks",
            "help",
            "identity",
            "time",
            "timers",
            "recall",
            "calendar",
            "system",
            "weather",
            "morning_brief",
        ] {
            assert!(registry.ids().contains(&id), "missing {id}");
        }
    }

    #[test]
    fn bridge_spoken_cancel() {
        let registry = sample_registry();
        let clock = FakeClock::at(chrono::Utc::now());
        let ctx = FeatureContext::new(&clock);
        let spoken = registry.execute_spoken("cancel", &ctx, &Value::Null).expect("spoken");
        assert_eq!(spoken, "Alright.");
    }

    #[test]
    fn unknown_id_errors() {
        let registry = sample_registry();
        let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap());
        let ctx = FeatureContext::new(&clock);
        let err = registry.execute("missing", &ctx, &Value::Null).expect_err("unknown");
        assert!(matches!(err, FeatureError::InvalidInput { .. }));
    }
}
