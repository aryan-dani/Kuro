use super::normalize::{has_word, normalize};

pub fn looks_like_daddys_home(text: &str) -> bool {
    is_daddys_home(&normalize(text))
}

pub fn matches(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let t = normalize(text);
    is_cancel(&t)
        || is_thanks(&t)
        || is_help(&t)
        || is_identity(&t)
        || is_time(&t)
        || is_date(&t)
        || is_pause(&t)
        || is_resume(&t)
        || is_stop_music(&t)
        || is_goodnight(&t)
        || is_daddys_home(&t)
        || is_power_off(&t)
        || is_power_on(&t)
        || is_news(&t)
        || is_inbox(&t)
        || is_meet(&t)
        || is_join_meet(&t)
        || is_whatsapp(&t)
        || is_workflow(&t)
        || is_workflow_hint(&t)
}

pub fn is_cancel(t: &str) -> bool {
    matches!(t, "cancel" | "never mind" | "nevermind" | "forget it" | "stop" | "abort")
}
pub fn is_thanks(t: &str) -> bool {
    ["thank you", "thanks", "cheers", "appreciate it"].iter().any(|p| t.contains(p))
}
pub fn is_help(t: &str) -> bool {
    // Exact / short help only. Do not substring-match "help me" inside real questions
    // like "can you help me build an LLM".
    matches!(
        t,
        "help"
            | "commands"
            | "help me"
            | "what can you do"
            | "your commands"
            | "what do you support"
            | "list commands"
            | "capabilities"
            | "can you help"
            | "can you help me"
    ) || capability_help_ask(t)
}

fn capability_help_ask(t: &str) -> bool {
    // Allow a short vocative prefix ("hey kuro", "as the newer kuro") before the
    // capability ask. Keep the word budget tight so real questions stay LLM.
    const PHRASES: &[&str] =
        &["what can you do", "list commands", "your commands", "what do you support", "capabilities"];
    if t.split_whitespace().count() > 14 {
        return false;
    }
    if ["can you help me", "help me build", "help me understand", "help me write", "help me debug"]
        .iter()
        .any(|p| t.contains(p))
    {
        return false;
    }
    PHRASES.iter().any(|p| t == *p || t.ends_with(p) || t.contains(&format!(" {p}")))
}
pub fn is_identity(t: &str) -> bool {
    ["who are you", "what are you", "your name", "whats your name"].iter().any(|p| t.contains(p))
}
pub fn is_date(t: &str) -> bool {
    t == "date"
        || t == "the date"
        || ["what day", "whats the date", "what date", "todays date", "current date", "what is today"]
            .iter()
            .any(|p| t.contains(p))
}
pub fn is_time(t: &str) -> bool {
    if is_date(t) && !t.contains("time") {
        return false;
    }
    t == "time"
        || t == "the time"
        || ["what time", "whats the time", "current time", "tell me the time", "time is it"]
            .iter()
            .any(|p| t.contains(p))
}
pub fn is_pause(t: &str) -> bool {
    if !t.contains("pause") {
        return false;
    }
    ["song", "music", "track", "audio", "media"].iter().any(|w| t.contains(w))
        || matches!(t, "pause" | "pause it" | "pause please" | "pause that")
}
pub fn is_resume(t: &str) -> bool {
    [
        "resume",
        "unpause",
        "continue the song",
        "continue music",
        "play the song",
        "play music",
        "keep playing",
        "resume music",
        "resume song",
    ]
    .iter()
    .any(|p| t.contains(p))
}
pub fn is_stop_music(t: &str) -> bool {
    ["stop", "close", "shut off", "shut down", "kill"].iter().any(|w| t.contains(w))
        && ["song", "music", "track", "clash", "media"].iter().any(|w| t.contains(w))
}
pub fn is_goodnight(t: &str) -> bool {
    t.contains("goodnight") || (t.contains("good") && t.contains("night")) || matches!(t, "night" | "sleep" | "bedtime")
}
pub fn is_daddys_home(t: &str) -> bool {
    let home = t.contains("home");
    let daddy = t.contains("daddy");
    let wake = t.contains("wakeup") || t.contains("wake");
    (wake && home) || (daddy && home) || (wake && daddy)
}
fn desk_keywords(t: &str) -> bool {
    ["light", "lights", "lamp", "plug", "charger", "monitor", "screen", "display", "desk", "board", "extension"]
        .iter()
        .any(|w| has_word(t, w))
}
fn power_command_shaped(t: &str) -> bool {
    // Imperative desk control, not "why does my monitor turn off".
    !crate::semantic::looks_like_question(t)
}
pub fn is_power_off(t: &str) -> bool {
    if !power_command_shaped(t) {
        return false;
    }
    if has_word(t, "on") && !has_word(t, "off") {
        return false;
    }
    if [
        "lights off",
        "light off",
        "plug off",
        "switch off",
        "power off",
        "power down",
        "monitor off",
        "screen off",
        "desk off",
        "display off",
    ]
    .iter()
    .any(|p| t.contains(p))
    {
        return true;
    }
    if has_word(t, "off") && desk_keywords(t) {
        return true;
    }
    t.contains("turn off") || t.contains("switch off")
}
pub fn is_power_on(t: &str) -> bool {
    if !power_command_shaped(t) {
        return false;
    }
    if has_word(t, "off") {
        return false;
    }
    if ["lights on", "light on", "plug on", "switch on", "power on", "monitor on", "screen on", "desk on", "display on"]
        .iter()
        .any(|p| t.contains(p))
    {
        return true;
    }
    if has_word(t, "on") && desk_keywords(t) {
        return true;
    }
    t.contains("turn on") || t.contains("switch on")
}
pub fn is_inbox(t: &str) -> bool {
    [
        "urgent mail",
        "urgent email",
        "any urgent",
        "inbox check",
        "check inbox",
        "check my inbox",
        "check my email",
        "check my mail",
        "check email",
        "check mail",
        "deadlines today",
        "any deadlines",
        "my deadlines",
        "important email",
        "important mail",
        "important emails",
        "missed email",
        "missed mail",
    ]
    .iter()
    .any(|p| t.contains(p))
}
pub fn is_news(t: &str) -> bool {
    if is_inbox(t) {
        return false;
    }
    [
        "news",
        "headlines",
        "whats happening",
        "whats the news",
        "what is the news",
        "brief me",
        "any updates",
        "latest news",
        "give me the news",
        "read the news",
    ]
    .iter()
    .any(|p| t.contains(p))
}
pub fn is_meet(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "make_meet")
}
pub fn is_join_meet(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "join_meet")
}
pub fn is_whatsapp(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "send_whatsapp")
}
pub fn is_workflow(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "run_workflow")
}
pub fn is_workflow_hint(t: &str) -> bool {
    t.contains("n8n")
        || t.contains("workflow")
        || (t.contains("automation") && ["run", "start", "trigger", "launch"].iter().any(|v| t.contains(v)))
}
pub fn is_music_only(t: &str) -> bool {
    is_pause(t) || is_resume(t) || is_stop_music(t)
}
pub fn is_hub_desk(t: &str) -> bool {
    is_daddys_home(t) || is_goodnight(t) || is_power_on(t) || is_power_off(t) || is_news(t)
}

/// Phrase-only intent label. Does not execute tools, speak, or touch the hub.
pub fn classify_intent(text: &str) -> &'static str {
    let t = normalize(text);
    if t.is_empty() {
        return "none";
    }
    if is_stop_music(&t) {
        return "stop_music";
    }
    if is_pause(&t) {
        return "pause";
    }
    if is_resume(&t) {
        return "resume";
    }
    if is_goodnight(&t) {
        return "goodnight";
    }
    if is_daddys_home(&t) {
        return "daddys_home";
    }
    if is_power_off(&t) {
        return "power_off";
    }
    if is_power_on(&t) {
        return "power_on";
    }
    if is_inbox(&t) {
        return "inbox";
    }
    if is_news(&t) {
        return "news";
    }
    if is_join_meet(&t) {
        return "join_meet";
    }
    if is_meet(&t) {
        return "meet";
    }
    if is_whatsapp(&t) {
        return "whatsapp";
    }
    if is_workflow(&t) || is_workflow_hint(&t) {
        return "workflow";
    }
    if is_cancel(&t) {
        return "cancel";
    }
    if is_thanks(&t) {
        return "thanks";
    }
    if is_help(&t) {
        return "help";
    }
    if is_identity(&t) {
        return "identity";
    }
    if is_date(&t) {
        return "date";
    }
    if is_time(&t) {
        return "time";
    }
    "none"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_accepts_vocative_capability_ask() {
        assert!(is_help("what can you do"));
        assert!(is_help("as the newer kuro what can you do"));
        assert!(is_help("hey kuro what can you do"));
        assert!(!is_help("can you help me build an llm"));
        assert!(!is_help("please help me understand borrow checker lifetimes"));
    }

    #[test]
    fn classify_desk_on_and_question_negative() {
        assert_eq!(classify_intent("desk on"), "power_on");
        assert_eq!(classify_intent("good night"), "goodnight");
        assert_eq!(classify_intent("why does my monitor turn off when I play games"), "none");
    }
}
