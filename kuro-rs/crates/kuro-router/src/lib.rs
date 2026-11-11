mod phrases;

pub use phrases::{
    PhraseResult, has_word, is_hub_desk, is_music_only, is_power_off, is_power_on, is_whatsapp, looks_like_daddys_home,
    matches, normalize, prefer_user_text, route_command, try_handle,
};
