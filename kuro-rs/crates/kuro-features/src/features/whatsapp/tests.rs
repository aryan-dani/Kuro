use std::sync::Arc;

use serde_json::json;

use super::{FakeWhatsappControl, WhatsappFeature};
use crate::context::{FeatureContext, SystemClock};
use crate::feature::Feature;

#[test]
fn noop_send_reply() {
    let feature = WhatsappFeature::default();
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "siya", "source": "browser_url", "text": ""})).expect("ok");
    assert_eq!(out.speech.as_str(), "Sent.");
}

#[test]
fn fake_records_slots() {
    let control = Arc::new(FakeWhatsappControl::default());
    let feature = WhatsappFeature::with_control(control.clone());
    let ctx = FeatureContext::new(&SystemClock);
    let slots = json!({"contact": "sam", "source": "clipboard", "text": "hello"});
    let _ = feature.execute(&ctx, &slots).expect("ok");
    let call = &control.recorded()[0];
    assert_eq!(call.contact, "sam");
    assert_eq!(call.source, "clipboard");
    assert_eq!(call.text, "hello");
}

#[test]
fn missing_contact_speaks_unknown() {
    let feature = WhatsappFeature::default();
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"source": "clipboard"})).expect("ok");
    assert_eq!(out.speech.as_str(), "I don't know who to message.");
}

#[test]
fn control_error_maps_clipboard_empty() {
    let control = Arc::new(FakeWhatsappControl::default());
    control.fail_with("The clipboard is empty. Copy text or an image first.");
    let feature = WhatsappFeature::with_control(control);
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "siya", "source": "clipboard", "text": ""})).expect("err");
    assert_eq!(out.speech.as_str(), "The clipboard is empty. Copy text or an image first.");
}

#[test]
fn control_error_unknown_source() {
    let control = Arc::new(FakeWhatsappControl::default());
    control.fail_with("I don't know how to send that.");
    let feature = WhatsappFeature::with_control(control);
    let ctx = FeatureContext::new(&SystemClock);
    let out = feature.execute(&ctx, &json!({"contact": "siya", "source": "fax", "text": ""})).expect("err");
    assert_eq!(out.speech.as_str(), "I don't know how to send that.");
}
