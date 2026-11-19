use serde_json::Value;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::identity::IdentityFeature;

#[test]
fn execute_introduces_kuro() {
    let feature = IdentityFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &Value::Null).expect("execute");
    assert_eq!(outcome.speech.as_str(), "Kuro. Local assistant. Phrases first, model last.");
}
