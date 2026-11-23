mod handle;
mod matchers;
mod normalize;
mod route;

pub use handle::try_handle;
pub use matchers::{
    classify_intent, is_cancel, is_hub_desk, is_music_only, is_power_off, is_power_on, is_whatsapp,
    looks_like_daddys_home, matches,
};
pub use normalize::{has_word, normalize};
pub use route::{RouteResult, prefer_user_text, route_command};

#[derive(Clone, Debug)]
pub struct PhraseResult {
    pub reply: String,
    pub skip_tts: bool,
    pub force_fast_tts: bool,
}
