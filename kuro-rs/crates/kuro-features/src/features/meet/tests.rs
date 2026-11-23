use std::sync::Arc;

use serde_json::json;

use super::{FakeMeetControl, MeetFeature};
use crate::context::{FeatureContext, SystemClock};
use crate::feature::Feature;

#[test]
fn noop_refuses_fake_success() {
    let feature = MeetFeature::default();
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": ""})).expect("ok");
    assert!(out.speech.as_str().contains("not wired") || out.speech.as_str().contains("join the meet"));
    let sent = feature.execute(&ctx, &json!({"contact": "mom"})).expect("ok");
    assert!(sent.speech.as_str().contains("not wired") || sent.speech.as_str().contains("make a meet"));
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

#[test]
fn join_opens_saved_meet() {
    let control = Arc::new(FakeMeetControl::default());
    let feature = MeetFeature::with_control(control.clone());
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "siya", "action": "join"})).expect("ok");
    assert_eq!(out.speech.as_str(), "The Meet is open.");
    assert_eq!(control.recorded()[0].action, "join");
}

#[test]
fn join_missing_link_maps() {
    let control = Arc::new(FakeMeetControl::default());
    control.fail_with("I don't have a Meet link for them.");
    let feature = MeetFeature::with_control(control);
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "siya", "action": "join"})).expect("err");
    assert_eq!(out.speech.as_str(), "I don't have a Meet link for them.");
}
