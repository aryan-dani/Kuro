//! Pure turn state machine (ADR 0008). No I/O in `step`; the engine drives ports.

pub mod machine;
pub mod runner;

pub use machine::{Effect, Event, HudMode, RouteOutcome, State, step};
pub use runner::TurnRunner;
