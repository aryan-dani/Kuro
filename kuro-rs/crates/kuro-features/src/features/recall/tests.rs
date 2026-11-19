use chrono::TimeZone;
use serde_json::json;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::recall::RecallFeature;

#[test]
fn repeat_speaks_last_utterance() {
    let feature = RecallFeature::default();
    feature.record_turn(Some("The weather is sunny."), None);
    let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "recall.repeat" })).expect("repeat");
    assert_eq!(outcome.speech.as_str(), "The weather is sunny.");
}

#[test]
fn repeat_empty_when_no_log() {
    let feature = RecallFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "recall.repeat" })).expect("repeat");
    assert_eq!(outcome.speech.as_str(), "I do not have anything to repeat yet.");
}

#[test]
fn last_question_speaks_saved_prompt() {
    let feature = RecallFeature::default();
    feature.record_turn(None, Some("What is on my calendar today?"));
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "recall.last_question" })).expect("last_question");
    assert_eq!(outcome.speech.as_str(), "You asked: What is on my calendar today?");
}
