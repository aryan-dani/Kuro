//! Typed settings: defaults, example profile, then environment.

mod env;
mod load;
mod settings;

pub use load::{ConfigLoadError, example_profile_path, repo_root, user_profile_path};
pub use settings::Settings;
