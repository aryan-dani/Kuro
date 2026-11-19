use std::sync::Arc;

use serde_json::json;

use super::{FakeMusicControl, MusicCall, MusicFeature};
use crate::context::{FeatureContext, SystemClock};
use crate::feature::Feature;

#[test]
fn pause_idle_reply() {
    let control = Arc::new(FakeMusicControl::default());
    let feature = MusicFeature::with_control(control.clone());
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let out = feature.execute(&ctx, &json!({"op": "pause"})).expect("ok");
    assert_eq!(out.speech.as_str(), "Nothing is playing.");
    assert_eq!(control.recorded(), vec![MusicCall::Pause]);
}

#[test]
fn resume_and_stop_idle_replies() {
    let feature = MusicFeature::default();
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let resume = feature.execute(&ctx, &json!({"op": "resume"})).expect("resume");
    assert_eq!(resume.speech.as_str(), "Nothing to resume.");
    let stop = feature.execute(&ctx, &json!({"op": "stop"})).expect("stop");
    assert_eq!(stop.speech.as_str(), "Music stopped.");
}

#[test]
fn fake_control_custom_replies() {
    let control = Arc::new(FakeMusicControl::default());
    control.set_pause_reply("Paused.");
    control.set_resume_reply("Resumed.");
    let feature = MusicFeature::with_control(control);
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let pause = feature.execute(&ctx, &json!({"op": "pause"})).expect("pause");
    assert_eq!(pause.speech.as_str(), "Paused.");
    let resume = feature.execute(&ctx, &json!({"op": "resume"})).expect("resume");
    assert_eq!(resume.speech.as_str(), "Resumed.");
}

#[test]
fn control_error_surfaces_speech() {
    let control = Arc::new(FakeMusicControl::default());
    control.fail_with("Music player is unavailable.");
    let feature = MusicFeature::with_control(control);
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let out = feature.execute(&ctx, &json!({"op": "pause"})).expect("err speech");
    assert_eq!(out.speech.as_str(), "Music player is unavailable.");
}

#[test]
fn unknown_op_is_invalid() {
    let feature = MusicFeature::default();
    let clock = SystemClock;
    let ctx = FeatureContext::new(&clock);
    let err = feature.execute(&ctx, &json!({"op": "shuffle"})).unwrap_err();
    assert!(err.to_string().contains("unknown music op"));
}
