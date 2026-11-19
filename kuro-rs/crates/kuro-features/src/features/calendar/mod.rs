mod ics;

use chrono::Local;
use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::Outcome;

pub use ics::{CalendarEvent, parse_events};

const MANIFEST_TOML: &str = include_str!("feature.toml");

#[derive(Debug)]
pub struct CalendarFeature {
    manifest: FeatureManifest,
}

impl CalendarFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("calendar feature.toml");
        Self { manifest }
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn intent(slots: &Value) -> Option<&str> {
        slots.get("intent").and_then(|v| v.as_str()).or_else(|| slots.get("action").and_then(|v| v.as_str()))
    }

    fn ics_text(ctx: &FeatureContext<'_>, slots: &Value) -> Option<String> {
        if let Some(text) = slots.get("ics_data").and_then(|v| v.as_str()) {
            return Some(text.to_string());
        }
        let path = slots.get("ics_path").and_then(|v| v.as_str()).or(ctx.calendar_ics_path)?;
        std::fs::read_to_string(path).ok()
    }

    fn format_local_time(iso: chrono::DateTime<chrono::Utc>) -> String {
        iso.with_timezone(&Local).format("%I:%M %p").to_string().trim_start_matches('0').replace(" 0", " ")
    }
}

impl Default for CalendarFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for CalendarFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let intent = Self::intent(slots).ok_or_else(|| {
            FeatureError::invalid_input("calendar feature needs slots.intent (calendar.next, calendar.today)")
        })?;
        let Some(ics) = Self::ics_text(ctx, slots) else {
            return Ok(Outcome::say(self.reply("not_configured", "Calendar is not configured.")));
        };
        let events = parse_events(&ics);
        let now = ctx.clock.now_utc();
        match intent {
            "calendar.next" => Ok(self.speak_next(&events, now)),
            "calendar.today" => Ok(self.speak_today(&events, now)),
            other => Err(FeatureError::invalid_input(format!("unknown calendar intent: {other}"))),
        }
    }
}

impl CalendarFeature {
    fn speak_next(&self, events: &[CalendarEvent], now: chrono::DateTime<chrono::Utc>) -> Outcome {
        let Some(event) = ics::next_event_after(events, now) else {
            return Outcome::say(self.reply("next_none", "You have no upcoming events."));
        };
        let template = self.reply("next_ok", "Next up: {summary} at {time}.");
        Outcome::say(
            template.replace("{summary}", &event.summary).replace("{time}", &Self::format_local_time(event.start)),
        )
    }

    fn speak_today(&self, events: &[CalendarEvent], now: chrono::DateTime<chrono::Utc>) -> Outcome {
        let day = now.with_timezone(&Local).date_naive();
        let today = ics::events_on_local_day(events, day, &Local);
        if today.is_empty() {
            return Outcome::say(self.reply("today_none", "Nothing on your calendar today."));
        }
        if today.len() == 1 {
            let event = today[0];
            let template = self.reply("today_one", "You have one event today: {summary} at {time}.");
            return Outcome::say(
                template.replace("{summary}", &event.summary).replace("{time}", &Self::format_local_time(event.start)),
            );
        }
        let template = self.reply("today_many", "You have {count} events today.");
        Outcome::say(template.replace("{count}", &today.len().to_string()))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
