use serde_json::Value;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::cancel::CancelFeature;

#[test]
fn execute_returns_alright() {
    let feature = CancelFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &Value::Null).expect("execute");
    assert_eq!(outcome.speech.as_str(), "Alright.");
}
