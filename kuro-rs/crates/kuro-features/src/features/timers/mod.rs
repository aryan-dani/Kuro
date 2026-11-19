mod parse;
mod store;

use std::sync::Mutex;

use kuro_core::FollowUp;
use serde_json::Value;

use crate::context::FeatureContext;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::manifest::{FeatureManifest, parse_manifest};
use crate::outcome::{Outcome, Spoken};

pub use parse::{format_duration, parse_duration};
pub use store::{ActiveTimer, StoreError, TimerStore};

const MANIFEST_TOML: &str = include_str!("feature.toml");
const DEFAULT_MAX_ACTIVE: usize = 10;

#[derive(Debug)]
pub struct TimersFeature {
    manifest: FeatureManifest,
    store: Mutex<TimerStore>,
    max_active: usize,
}

impl TimersFeature {
    fn loaded() -> Self {
        let manifest = parse_manifest(MANIFEST_TOML).expect("timers feature.toml");
        Self { manifest, store: Mutex::new(TimerStore::default()), max_active: DEFAULT_MAX_ACTIVE }
    }

    pub fn with_max_active(max_active: usize) -> Self {
        let mut feature = Self::loaded();
        feature.max_active = max_active.max(1);
        feature
    }

    /// Poll in-memory timers against `ctx.clock` (first slice; Scheduler port lands later).
    pub fn poll_due(&self, ctx: &FeatureContext<'_>) -> Vec<Outcome> {
        let now = ctx.clock.now_utc();
        let done = self.reply("timer_done", "Your timer is done.");
        let mut store = self.store.lock().expect("timers store");
        store.due(now).into_iter().map(|_| Outcome::say(done.clone())).collect()
    }

    fn reply(&self, key: &str, fallback: &str) -> String {
        self.manifest.replies.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn ask(&self, key: &str, fallback: &str) -> Outcome {
        Outcome { speech: Spoken::new(self.reply(key, fallback)), follow_up: FollowUp::Listen, data: None }
    }

    fn slot_str<'a>(slots: &'a Value, key: &str) -> Option<&'a str> {
        slots.get(key).and_then(|v| v.as_str())
    }

    fn intent(slots: &Value) -> Option<&str> {
        Self::slot_str(slots, "intent").or_else(|| Self::slot_str(slots, "action"))
    }
}

impl Default for TimersFeature {
    fn default() -> Self {
        Self::loaded()
    }
}

impl Feature for TimersFeature {
    fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    fn manifest(&self) -> &FeatureManifest {
        &self.manifest
    }

    fn execute(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let intent = Self::intent(slots).ok_or_else(|| {
            FeatureError::invalid_input("timers feature needs slots.intent (timers.set, timers.cancel, timers.list)")
        })?;
        match intent {
            "timers.set" => self.execute_set(ctx, slots),
            "timers.cancel" => self.execute_cancel(slots),
            "timers.list" => Ok(self.execute_list()),
            other => Err(FeatureError::invalid_input(format!("unknown timers intent: {other}"))),
        }
    }
}

impl TimersFeature {
    fn execute_set(&self, ctx: &FeatureContext<'_>, slots: &Value) -> Result<Outcome, FeatureError> {
        let raw = Self::slot_str(slots, "duration").unwrap_or("");
        let Some(duration) = parse_duration(raw) else {
            return Ok(self.ask("ask_duration", "For how long should I set the timer?"));
        };
        let label = Self::slot_str(slots, "label").map(str::to_string);
        let now = ctx.clock.now_utc();
        let mut store = self.store.lock().expect("timers store");
        if store.add(duration, label, now, self.max_active).is_err() {
            return Ok(Outcome::say(
                self.reply("limit_reached", "You already have the maximum number of timers running."),
            ));
        }
        let speech =
            self.reply("set_ok", "Timer set for {duration}.").replace("{duration}", &format_duration(duration));
        Ok(Outcome::say(speech))
    }

    fn execute_cancel(&self, slots: &Value) -> Result<Outcome, FeatureError> {
        let label = Self::slot_str(slots, "label");
        let mut store = self.store.lock().expect("timers store");
        let n = store.cancel(label);
        if n == 0 {
            Ok(Outcome::say(self.reply("cancel_none", "No timers are running.")))
        } else {
            Ok(Outcome::say(self.reply("cancel_ok", "Timer canceled.")))
        }
    }

    fn execute_list(&self) -> Outcome {
        let count = self.store.lock().expect("timers store").active_count();
        if count == 0 {
            return Outcome::say(self.reply("list_empty", "No timers are running."));
        }
        if count == 1 {
            return Outcome::say(self.reply("list_one", "One timer is running."));
        }
        let template = self.reply("list_many", "{count} timers are running.");
        Outcome::say(template.replace("{count}", &count.to_string()))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
