//! Engine driver loop lands in a later slice (phase 12 wiring).
//!
//! The driver will: read `(State, Vec<Effect>)` from `step`, dispatch each effect
//! through `kuro-core` ports, map async completions to `Event`, and honor epoch drops.

use crate::{Effect, Event, State, step};

/// Placeholder for the future single-threaded turn driver.
pub struct TurnRunner {
    state: State,
}

impl TurnRunner {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    pub fn state(&self) -> State {
        self.state.clone()
    }

    pub fn on_event(&mut self, event: Event) -> Vec<Effect> {
        let (next, effects) = step(self.state.clone(), event);
        self.state = next;
        effects
    }
}

impl Default for TurnRunner {
    fn default() -> Self {
        Self::new()
    }
}
