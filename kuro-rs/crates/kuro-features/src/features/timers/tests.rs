use chrono::TimeZone;
use kuro_core::FollowUp;
use serde_json::json;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::timers::TimersFeature;

fn slots(intent: &str, duration: &str) -> serde_json::Value {
    json!({ "intent": intent, "duration": duration })
}

#[test]
fn fake_clock_fires_timer() {
    let feature = TimersFeature::default();
    let start = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap();
    let clock = FakeClock::at(start);
    let ctx = FeatureContext::new(&clock);
    feature.execute(&ctx, &slots("timers.set", "1 minute")).expect("set");

    let later = FakeClock::at(start + chrono::Duration::minutes(1));
    let later_ctx = FeatureContext::new(&later);
    let fired = feature.poll_due(&later_ctx);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].speech.as_str(), "Your timer is done.");

    let again = feature.poll_due(&later_ctx);
    assert!(again.is_empty());
}

#[test]
fn unparsable_duration_asks() {
    let feature = TimersFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &slots("timers.set", "soon")).expect("execute");
    assert_eq!(outcome.follow_up, FollowUp::Listen);
    assert_eq!(outcome.speech.as_str(), "For how long should I set the timer?");
}

#[test]
fn max_active_limit() {
    let feature = TimersFeature::with_max_active(2);
    let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    let ctx = FeatureContext::new(&clock);
    feature.execute(&ctx, &slots("timers.set", "1 minute")).expect("first");
    feature.execute(&ctx, &slots("timers.set", "2 minutes")).expect("second");
    let third = feature.execute(&ctx, &slots("timers.set", "3 minutes")).expect("third");
    assert_eq!(third.speech.as_str(), "You already have the maximum number of timers running.");
}
