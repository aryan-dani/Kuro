//! Feature extension model: manifest data, registry, and sample features.

mod bridge;
mod context;
mod dynamic;
mod error;
mod feature;
mod manifest;
mod outcome;
mod registry;

pub mod features;

pub use bridge::{
    execute_spoken, execute_spoken_registry, feature_id_for_intent, spoken_for_intent, spoken_for_intent_with_slots,
};
pub use context::{Clock, FakeClock, FeatureContext, SystemClock};
pub use dynamic::DynamicSource;
pub use error::FeatureError;
pub use feature::Feature;
pub use manifest::{FeatureManifest, ManifestError, parse_manifest};
pub use outcome::{Outcome, Spoken};
pub use registry::{FeatureRegistry, RegistryError};
