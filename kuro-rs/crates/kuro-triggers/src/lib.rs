//! Activation sources (hotkey wrapper, wake word). Engine wiring lands later.

mod fake;
mod null;
mod registry;
mod trigger;

pub use fake::FakeDetector;
pub use null::NullWakeWord;
pub use registry::TriggerRegistry;
pub use trigger::{Activation, Trigger};
