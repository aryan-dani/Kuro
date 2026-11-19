//! Checked-in registry: one `pub mod` and one `register` line per feature.

use crate::registry::FeatureRegistry;

// scaffold:mod
pub mod calendar;
pub mod cancel;
pub mod desk;
pub mod help;
pub mod identity;
pub mod meet;
pub mod morning_brief;
pub mod music;
pub mod recall;
pub mod system;
pub mod thanks;
pub mod time;
pub mod timers;
pub mod weather;
pub mod whatsapp;

pub use calendar::CalendarFeature;
pub use cancel::CancelFeature;
pub use desk::DeskFeature;
pub use help::HelpFeature;
pub use identity::IdentityFeature;
pub use meet::MeetFeature;
pub use morning_brief::MorningBriefFeature;
pub use music::MusicFeature;
pub use recall::RecallFeature;
pub use system::SystemFeature;
pub use thanks::ThanksFeature;
pub use time::TimeFeature;
pub use timers::TimersFeature;
pub use weather::WeatherFeature;
pub use whatsapp::WhatsappFeature;

pub fn register_all(registry: &mut FeatureRegistry) {
    // scaffold:register
    registry.register(Box::new(cancel::CancelFeature::default()));
    registry.register(Box::new(thanks::ThanksFeature::default()));
    registry.register(Box::new(help::HelpFeature::default()));
    registry.register(Box::new(identity::IdentityFeature::default()));
    registry.register(Box::new(time::TimeFeature::default()));
    registry.register(Box::new(timers::TimersFeature::default()));
    registry.register(Box::new(recall::RecallFeature::default()));
    registry.register(Box::new(calendar::CalendarFeature::default()));
    registry.register(Box::new(system::SystemFeature::default()));
    registry.register(Box::new(weather::WeatherFeature::default()));
    registry.register(Box::new(morning_brief::MorningBriefFeature::default()));
    registry.register(Box::new(desk::DeskFeature::default()));
    registry.register(Box::new(music::MusicFeature::default()));
    registry.register(Box::new(meet::MeetFeature::default()));
    registry.register(Box::new(whatsapp::WhatsappFeature::default()));
}
