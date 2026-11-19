use serde_json::json;

use crate::context::{FakeClock, FeatureContext};
use crate::feature::Feature;
use crate::features::weather::WeatherFeature;

const FIXTURE_JSON: &str = r#"{"current_weather":{"temperature":18.0,"windspeed":12.0,"weathercode":2}}"#;

#[test]
fn now_from_context_weather_json() {
    let feature = WeatherFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::with_weather_json(&clock, FIXTURE_JSON);
    let outcome = feature.execute(&ctx, &json!({ "intent": "weather.now" })).expect("weather");
    assert_eq!(outcome.speech.as_str(), "It is 18 degrees.");
}

#[test]
fn now_from_slots_weather_json() {
    let feature = WeatherFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome =
        feature.execute(&ctx, &json!({ "intent": "weather.now", "weather_json": FIXTURE_JSON })).expect("weather");
    assert_eq!(outcome.speech.as_str(), "It is 18 degrees.");
}

#[test]
fn not_configured_without_data() {
    let feature = WeatherFeature::default();
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    let outcome = feature.execute(&ctx, &json!({ "intent": "weather.now" })).expect("weather");
    assert_eq!(outcome.speech.as_str(), "Weather is not configured. Set a location in your profile.");
}
