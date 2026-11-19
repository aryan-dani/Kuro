//! Ports passed into `Feature::execute`. Stubs expand in later phases.

use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug, Clone)]
pub struct FakeClock {
    fixed: DateTime<Utc>,
}

impl FakeClock {
    pub fn at(fixed: DateTime<Utc>) -> Self {
        Self { fixed }
    }
}

impl Clock for FakeClock {
    fn now_utc(&self) -> DateTime<Utc> {
        self.fixed
    }
}

pub struct FeatureContext<'a> {
    pub clock: &'a dyn Clock,
    pub calendar_ics_path: Option<&'a str>,
    pub weather_json: Option<&'a str>,
}

impl<'a> FeatureContext<'a> {
    pub fn new(clock: &'a dyn Clock) -> Self {
        Self { clock, calendar_ics_path: None, weather_json: None }
    }

    pub fn with_calendar_ics_path(clock: &'a dyn Clock, path: &'a str) -> Self {
        Self { clock, calendar_ics_path: Some(path), weather_json: None }
    }

    pub fn with_weather_json(clock: &'a dyn Clock, weather_json: &'a str) -> Self {
        Self { clock, calendar_ics_path: None, weather_json: Some(weather_json) }
    }

    pub fn with_brief_inputs(
        clock: &'a dyn Clock,
        calendar_ics_path: Option<&'a str>,
        weather_json: Option<&'a str>,
    ) -> Self {
        Self { clock, calendar_ics_path, weather_json }
    }
}
