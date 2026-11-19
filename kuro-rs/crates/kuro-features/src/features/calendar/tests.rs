use chrono::{Local, TimeZone};
use serde_json::json;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::calendar::CalendarFeature;

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
BEGIN:VEVENT
DTSTART:20260616T090000Z
SUMMARY:Roadmap sync
END:VEVENT
END:VCALENDAR";

fn slots(intent: &str) -> serde_json::Value {
    json!({ "intent": intent, "ics_data": FIXTURE_ICS })
}

#[test]
fn not_configured_without_ics() {
    let feature = CalendarFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "calendar.next" })).expect("next");
    assert_eq!(outcome.speech.as_str(), "Calendar is not configured.");
}

#[test]
fn next_event_from_fixture() {
    let feature = CalendarFeature::default();
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
    let clock = FakeClock::at(now);
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &slots("calendar.next")).expect("next");
    let local = chrono::Utc
        .with_ymd_and_hms(2026, 6, 15, 14, 0, 0)
        .unwrap()
        .with_timezone(&Local)
        .format("%I:%M %p")
        .to_string()
        .trim_start_matches('0')
        .replace(" 0", " ");
    let expected = format!("Next up: Design review at {local}.");
    assert_eq!(outcome.speech.as_str(), expected);
}

#[test]
fn today_counts_events() {
    let feature = CalendarFeature::default();
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 15, 8, 0, 0).unwrap();
    let clock = FakeClock::at(now);
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &slots("calendar.today")).expect("today");
    assert_eq!(outcome.speech.as_str(), "You have 2 events today.");
}
