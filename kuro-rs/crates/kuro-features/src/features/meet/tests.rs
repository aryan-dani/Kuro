use std::sync::Arc;

use serde_json::json;

use super::{FakeMeetControl, MeetFeature};
use crate::context::{FeatureContext, SystemClock};
use crate::feature::Feature;

#[test]
fn noop_open_meet() {
    let feature = MeetFeature::default();
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": ""})).expect("ok");
    assert_eq!(out.speech.as_str(), "The Meet is open.");
}

#[test]
fn noop_send_to_contact() {
    let feature = MeetFeature::default();
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "mom"})).expect("ok");
    assert_eq!(out.speech.as_str(), "Sent the Meet link to mom.");
}

#[test]
fn fake_control_records_contact() {
    let control = Arc::new(FakeMeetControl::default());
    let feature = MeetFeature::with_control(control.clone());
    let ctx = FeatureContext::new(&SystemClock);
    let _ = feature.execute(&ctx, &json!({"contact": "mob"})).expect("ok");
    assert_eq!(control.recorded()[0].contact, "mob");
}

#[test]
fn control_error_maps_unknown_contact() {
    let control = Arc::new(FakeMeetControl::default());
    control.fail_with("I don't know that contact.");
    let feature = MeetFeature::with_control(control);
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "x"})).expect("err");
    assert_eq!(out.speech.as_str(), "I don't know that contact.");
}

#[test]
fn control_error_maps_busy() {
    let control = Arc::new(FakeMeetControl::default());
    control.fail_with("I'm already opening a Meet.");
    let feature = MeetFeature::with_control(control);
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": ""})).expect("err");
    assert_eq!(out.speech.as_str(), "I'm already opening a Meet.");
}
