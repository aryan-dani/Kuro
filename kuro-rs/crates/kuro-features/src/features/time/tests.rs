use chrono::TimeZone;
use serde_json::Value;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::time::{TimeFeature, format_clock_reply};

#[test]
fn format_matches_router_style() {
    let local = chrono::Local.with_ymd_and_hms(2026, 6, 15, 14, 30, 0).unwrap();
    assert_eq!(format_clock_reply(local), "It's 2:30 PM.");
}

#[test]
fn execute_uses_clock_port() {
    let feature = TimeFeature::default();
    let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 6, 15, 9, 0, 0).unwrap());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &Value::Null).expect("execute");
    assert!(outcome.speech.as_str().starts_with("It's "));
}
