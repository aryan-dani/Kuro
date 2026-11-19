use chrono::TimeZone;
use serde_json::json;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::morning_brief::MorningBriefFeature;

const FIXTURE_JSON: &str = r#"{"current_weather":{"temperature":18.0,"windspeed":12.0,"weathercode":2}}"#;

const FIXTURE_ICS: &str = "\
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
DTSTART:20260615T100000Z
SUMMARY:Team standup
END:VEVENT
BEGIN:VEVENT
DTSTART:20260615T140000Z
SUMMARY:Design review
END:VEVENT
END:VCALENDAR";

fn brief_slots(extra: serde_json::Value) -> serde_json::Value {
    let mut base = json!({
        "intent": "morning_brief.run",
        "weather_json": FIXTURE_JSON,
        "ics_data": FIXTURE_ICS
    });
    if let (Some(a), Some(b)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in b {
            a.insert(k.clone(), v.clone());
        }
    }
    base
}

#[test]
fn composes_weather_and_calendar() {
    let feature = MorningBriefFeature::default();
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 15, 8, 0, 0).unwrap();
    let clock = FakeClock::at(now);
    let ctx = FeatureContext::with_brief_inputs(&clock, None, Some(FIXTURE_JSON));
    let outcome = feature.execute(&ctx, &brief_slots(json!({}))).expect("brief");
    let speech = outcome.speech.as_str();
    assert!(speech.contains("Good morning."));
    assert!(speech.contains("18 degrees"));
    assert!(speech.contains("2 events today"));
}

#[test]
fn weather_missing_still_returns_calendar() {
    let feature = MorningBriefFeature::default();
    let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 6, 15, 8, 0, 0).unwrap());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature
        .execute(
            &ctx,
            &json!({
                "intent": "morning_brief.run",
                "ics_data": FIXTURE_ICS
            }),
        )
        .expect("brief");
    let speech = outcome.speech.as_str();
    assert!(!speech.contains("degrees"));
    assert!(speech.contains("events today"));
}

#[test]
fn calendar_missing_still_returns_weather() {
    let feature = MorningBriefFeature::default();
    let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 6, 15, 8, 0, 0).unwrap());
    let ctx = FeatureContext::with_weather_json(&clock, FIXTURE_JSON);
    let outcome =
        feature.execute(&ctx, &json!({ "intent": "morning_brief.run", "weather_json": FIXTURE_JSON })).expect("brief");
    let speech = outcome.speech.as_str();
    assert!(speech.contains("18 degrees"));
    assert!(!speech.contains("events today"));
}

#[test]
fn optional_recall_appended_when_requested() {
    let feature = MorningBriefFeature::default();
    feature.record_recall("what is on my calendar");
    let clock = FakeClock::at(chrono::Utc.with_ymd_and_hms(2026, 6, 15, 8, 0, 0).unwrap());
    let ctx = FeatureContext::with_brief_inputs(&clock, None, Some(FIXTURE_JSON));
    let outcome = feature.execute(&ctx, &brief_slots(json!({ "include_recall": true }))).expect("brief");
    assert!(outcome.speech.as_str().contains("You asked: what is on my calendar"));
}

#[test]
fn disabled_flag_speaks_hint() {
    let feature = MorningBriefFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "morning_brief.run", "enabled": false })).expect("brief");
    assert_eq!(outcome.speech.as_str(), "Morning brief is turned off in your profile.");
}

#[test]
fn all_sources_missing_gets_setup_hint() {
    let feature = MorningBriefFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "morning_brief.run" })).expect("brief");
    let speech = outcome.speech.as_str();
    assert!(speech.contains("Good morning."));
    assert!(speech.contains("empty"));
}
