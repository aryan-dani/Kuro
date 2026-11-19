use std::sync::Arc;

use serde_json::json;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::system::{FakeSystemControl, SystemCall, SystemFeature};

#[test]
fn volume_set_records_fake_control() {
    let fake = Arc::new(FakeSystemControl::default());
    let feature = SystemFeature::with_control(fake.clone());
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "system.volume_set", "level": 30 })).expect("volume");
    assert_eq!(outcome.speech.as_str(), "Volume set to 30 percent.");
    assert_eq!(fake.recorded(), vec![SystemCall::VolumeSet(30)]);
}

#[test]
fn mute_records_fake_control() {
    let fake = Arc::new(FakeSystemControl::default());
    let feature = SystemFeature::with_control(fake.clone());
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    feature.execute(&ctx, &json!({ "intent": "system.mute" })).expect("mute");
    assert_eq!(fake.recorded(), vec![SystemCall::Mute]);
}

#[test]
fn lock_records_fake_control() {
    let fake = Arc::new(FakeSystemControl::default());
    let feature = SystemFeature::with_control(fake.clone());
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    feature.execute(&ctx, &json!({ "intent": "system.lock" })).expect("lock");
    assert_eq!(fake.recorded(), vec![SystemCall::Lock]);
}
