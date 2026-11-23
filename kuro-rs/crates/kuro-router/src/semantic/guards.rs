use super::Intent;

/// Polite help suffix on a longer utterance → LLM, never Help.
pub fn conversational_help_suffix(normalized: &str) -> bool {
    let words = normalized.split_whitespace().count();
    if words < 8 {
        return false;
    }
    ["can you help me", "help me out", "please help", "can you help", "help me", "please help me"]
        .iter()
        .any(|s| normalized.ends_with(s) || normalized.contains(&format!("{s} ")))
}

/// Question-shaped utterances that must not steal desk/power tools.
pub fn looks_like_question(normalized: &str) -> bool {
    let first = normalized.split_whitespace().next().unwrap_or("");
    if matches!(first, "why" | "how") && normalized.split_whitespace().count() >= 5 {
        return true;
    }
    // Conversational help / explanation requests that embed tool keywords.
    if conversational_help_suffix(normalized) {
        return true;
    }
    if normalized.contains("when i ") || normalized.contains("when i play") {
        return true;
    }
    // Short adversarial help embeds ("borrow checker can you help me").
    let words = normalized.split_whitespace().count();
    (5..=12).contains(&words)
        && (normalized.contains("can you help")
            || normalized.ends_with("help me")
            || normalized.ends_with("help me out"))
}

/// JoinMeet must look like join/reopen, not create/start/send.
pub(crate) fn join_meet_shaped(normalized: &str) -> bool {
    let t = normalized.trim();
    t.starts_with("join ")
        || t.starts_with("reopen ")
        || (t.starts_with("open ")
            && (t.contains("sent") || t.contains("earlier") || t.contains("already") || t.contains("that meet")))
}

pub(crate) fn polarity_ok(intent: Intent, normalized: &str) -> bool {
    let on = ["on", "up", "wake", "start", "enable", "boot"].iter().any(|w| crate::has_word(normalized, w));
    let off = ["off", "down", "kill", "shut", "disable", "sleep"].iter().any(|w| crate::has_word(normalized, w));
    match intent {
        Intent::PowerOn | Intent::DaddyHome => on || !off,
        Intent::PowerOff | Intent::GoodNight => off || !on,
        Intent::JoinMeet => join_meet_shaped(normalized),
        _ => true,
    }
}
