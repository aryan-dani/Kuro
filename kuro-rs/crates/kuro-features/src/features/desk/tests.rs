use std::sync::Arc;

use serde_json::json;

use super::{DeskCall, DeskFeature, FakeDeskControl};
use crate::context::{FeatureContext, SystemClock};
use crate::feature::Feature;

#[test]
fn power_on_reply() {
    let control = Arc::new(FakeDeskControl::default());
    let feature = DeskFeature::with_control(control.clone());
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let out = feature.execute(&ctx, &json!({"scene": "power_on"})).expect("ok");
    assert_eq!(out.speech.as_str(), "Desk on.");
    assert_eq!(control.recorded(), vec![DeskCall::Scene("power_on".into())]);
}

#[test]
fn power_off_and_good_night_replies() {
    let feature = DeskFeature::default();
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let off = feature.execute(&ctx, &json!({"scene": "power_off"})).expect("off");
    assert_eq!(off.speech.as_str(), "Desk off.");
    let night = feature.execute(&ctx, &json!({"scene": "good_night"})).expect("night");
    assert_eq!(night.speech.as_str(), "Desk off. Good night, sir.");
}

#[test]
fn missing_credentials_surface_raw() {
    let control = Arc::new(FakeDeskControl::default());
    control.fail_with("Smart plug credentials are missing in .env.");
    let feature = DeskFeature::with_control(control);
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let out = feature.execute(&ctx, &json!({"scene": "power_on"})).expect("err speech");
    assert!(out.speech.as_str().contains("missing"));
}

#[test]
fn unknown_scene_is_invalid() {
    let feature = DeskFeature::default();
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let err = feature.execute(&ctx, &json!({"scene": "blink"})).unwrap_err();
    assert!(err.to_string().contains("unknown desk scene"));
}
