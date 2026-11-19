use serde_json::Value;

use crate::context::FeatureContext;
use crate::feature::Feature;
use crate::features::{{id}}::{{type_name}};

#[test]
fn execute_returns_ok_reply() {
    let feature = {{type_name}}::default();
    let clock = crate::context::FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &Value::Null).expect("execute");
    assert_eq!(outcome.speech.as_str(), "Done.");
}