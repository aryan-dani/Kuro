use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveTimer {
    pub id: u64,
    pub label: Option<String>,
    pub started_at: DateTime<Utc>,
    pub fires_at: DateTime<Utc>,
    pub fired: bool,
}

#[derive(Debug, Default)]
pub struct TimerStore {
    next_id: u64,
    timers: Vec<ActiveTimer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreError {
    LimitReached,
    NotFound,
}

impl TimerStore {
    pub fn active_count(&self) -> usize {
        self.timers.iter().filter(|t| !t.fired).count()
    }

    pub fn add(
        &mut self,
        duration: chrono::Duration,
        label: Option<String>,
        now: DateTime<Utc>,
        max_active: usize,
    ) -> Result<ActiveTimer, StoreError> {
        if self.active_count() >= max_active {
            return Err(StoreError::LimitReached);
        }
        self.next_id = self.next_id.saturating_add(1);
        let timer = ActiveTimer { id: self.next_id, label, started_at: now, fires_at: now + duration, fired: false };
        self.timers.push(timer.clone());
        Ok(timer)
    }

    pub fn cancel(&mut self, label: Option<&str>) -> usize {
        let mut canceled = 0usize;
        for timer in &mut self.timers {
            if timer.fired {
                continue;
            }
            let matches = match label {
                None => true,
                Some(want) => timer.label.as_deref().is_some_and(|got| got.eq_ignore_ascii_case(want)),
            };
            if matches {
                timer.fired = true;
                canceled += 1;
            }
        }
        canceled
    }

    pub fn due(&mut self, now: DateTime<Utc>) -> Vec<ActiveTimer> {
        let mut fired = Vec::new();
        for timer in &mut self.timers {
            if !timer.fired && now >= timer.fires_at {
                timer.fired = true;
                fired.push(timer.clone());
            }
        }
        fired
    }
}
