//! Soft Meet follow-up understanding: referential language over exact phrase lists.

use crate::meet_last;
use fancy_regex::Regex;
use std::collections::HashMap;

fn normalize(text: &str) -> String {
    let norm = crate::n8n::normalize(text);
    let fillers = ["please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"];
    let mut toks: Vec<&str> = norm.split_whitespace().collect();
    while toks.first().is_some_and(|t| fillers.contains(t)) {
        toks.remove(0);
    }
    toks.join(" ")
}

fn clean_name(name: &str) -> String {
    let re = Regex::new(r"\s+(?:on|in|via)\s+whatsapp$").unwrap();
    let name = re.replace(name.trim(), "").to_string();
    let fillers = ["please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"];
    let mut toks: Vec<&str> = name.split_whitespace().collect();
    while toks.last().is_some_and(|t| fillers.contains(t)) {
        toks.pop();
    }
    toks.join(" ")
}

fn has_meet_word(body: &str) -> bool {
    body.split_whitespace().any(|w| matches!(w, "meet" | "mate" | "meat" | "mete"))
}

fn extract_contact(body: &str) -> String {
    for marker in ["you sent up to ", "you sent to ", "sent up to ", "sent to ", " for "] {
        if let Some(idx) = body.rfind(marker) {
            let rest = body[idx + marker.len()..].trim();
            if rest.is_empty() {
                continue;
            }
            let name = clean_name(rest);
            if !name.is_empty() && !has_meet_word(&name) {
                return name;
            }
        }
    }
    String::new()
}

fn looks_like_create_or_send(body: &str) -> bool {
    body.starts_with("make ")
        || body.starts_with("create ")
        || body.starts_with("start ")
        || (body.starts_with("send ") && !body.contains("send me into"))
}

fn join_signal(body: &str) -> bool {
    [
        "join ",
        "joined",
        "haven't joined",
        "have not joined",
        "reopen ",
        "get into ",
        "get me into ",
        "pull up ",
        "open that ",
        "open the ",
        "open my ",
    ]
    .iter()
    .any(|s| body.contains(s))
}

fn referential_signal(body: &str) -> bool {
    [
        "you sent",
        "sent to",
        "sent up",
        "that meet",
        "the meet",
        "the meet link",
        "that link",
        "from earlier",
        "already opened",
        "the one you",
        "same meet",
        "same link",
    ]
    .iter()
    .any(|s| body.contains(s))
}

/// Soft join follow-up when the user points at a Meet Kuro already made or sent.
pub fn match_followup(text: &str) -> Option<HashMap<String, String>> {
    let body = normalize(text);
    if body.is_empty() || looks_like_create_or_send(&body) {
        return None;
    }
    if !has_meet_word(&body) {
        return None;
    }
    if !(join_signal(&body) || referential_signal(&body)) {
        return None;
    }
    // Bare "open a meet" is create-shaped; require referential or join wording.
    if body.starts_with("open ") && !referential_signal(&body) && !body.contains("join") {
        return None;
    }
    let mut out = HashMap::new();
    out.insert("action".into(), "join".into());
    out.insert("contact".into(), extract_contact(&body));
    Some(out)
}

/// Short system hint for the conversational brain (no personal data in repo; runtime only).
pub fn context_blurb() -> Option<String> {
    let path = meet_last::store_path();
    let store = meet_last::load(&path);
    let last = store.last.as_ref()?;
    if last.url.trim().is_empty() {
        return None;
    }
    let who = if last.contact.trim().is_empty() { "someone".into() } else { last.contact.clone() };
    Some(format!(
        "Recent Meet context: Kuro already has a Meet link for {who}. If the user asks to join, open, or reopen that Meet, the Meet tool should handle it."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_followup_catches_natural_join() {
        let got = match_followup("hey kuro get me into that google meet you sent to siya").unwrap();
        assert_eq!(got.get("action").map(String::as_str), Some("join"));
        assert_eq!(got.get("contact").map(String::as_str), Some("siya"));
    }

    #[test]
    fn soft_followup_ignores_whatsapp_send() {
        assert!(match_followup("send this link to siya on whatsapp").is_none());
        assert!(match_followup("send this link to siya").is_none());
    }

    #[test]
    fn soft_followup_ignores_create() {
        assert!(match_followup("make a google meet").is_none());
        assert!(match_followup("start a meet").is_none());
    }

    #[test]
    fn soft_followup_catches_why_havent_joined() {
        let got = match_followup("why haven't I joined the meet").unwrap();
        assert_eq!(got.get("action").map(String::as_str), Some("join"));
    }
}
