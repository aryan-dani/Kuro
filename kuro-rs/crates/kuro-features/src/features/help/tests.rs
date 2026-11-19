use serde_json::{Value, json};

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::help::HelpFeature;
use crate::registry::FeatureRegistry;

#[test]
fn execute_lists_capabilities() {
    let feature = HelpFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &Value::Null).expect("execute");
    assert!(outcome.speech.as_str().contains("Desk, music"));
}

#[test]
fn what_can_i_say_from_feature_ids() {
    let feature = HelpFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature
        .execute(
            &ctx,
            &json!({
                "intent": "help.what_can_i_say",
                "feature_ids": ["cancel", "weather"]
            }),
        )
        .expect("discover");
    assert!(outcome.speech.as_str().contains("cancel:"));
    assert!(outcome.speech.as_str().contains("weather:"));
}

#[test]
fn what_can_i_say_from_registry_snapshot() {
    let registry = FeatureRegistry::with_features();
    let snapshot: Vec<_> = registry.ids().into_iter().map(|id| json!({ "id": id, "title": id })).collect();
    let feature = HelpFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature
        .execute(
            &ctx,
            &json!({
                "intent": "help.what_can_i_say",
                "registry_snapshot": snapshot
            }),
        )
        .expect("discover");
    assert!(outcome.speech.as_str().contains("weather"));
    assert!(outcome.speech.as_str().contains("cancel"));
}
