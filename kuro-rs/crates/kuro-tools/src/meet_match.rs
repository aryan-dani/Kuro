//! Phrase matching for create/send Meet. Keeps STT typos from falling through to chat.

use crate::meet_followup;
use fancy_regex::Regex;
use std::collections::HashMap;

fn normalize(text: &str) -> String {
    let norm = crate::n8n::normalize(text);
    let fillers = ["please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"];
    let mut toks: Vec<String> = norm
        .split_whitespace()
        .map(|t| t.trim_start_matches(|c: char| !c.is_ascii_alphanumeric()).to_string())
        .filter(|t| !t.is_empty())
        .collect();
    while toks.first().is_some_and(|t| fillers.contains(&t.as_str())) {
        toks.remove(0);
    }
    while toks.last().is_some_and(|t| fillers.contains(&t.as_str())) {
        toks.pop();
    }
    // Whisper often hears "to" as "td" / "too" before a contact name.
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].as_str();
        let prev_meetish = i > 0 && matches!(toks[i - 1].as_str(), "link" | "meet" | "mate" | "meat" | "mete" | "it");
        if prev_meetish && matches!(t, "td" | "too" | "two" | "2") {
            out.push("to".into());
        } else {
            out.push(toks[i].clone());
        }
        i += 1;
    }
    out.join(" ")
}

fn clean_name(name: &str) -> String {
    let re = Regex::new(r"\s+(?:on|in|via)\s+whatsapp$").unwrap();
    let name = re.replace(name.trim(), "").to_string();
    let fillers = ["please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly", "for", "pas"];
    let mut toks: Vec<&str> = name.split_whitespace().collect();
    while toks.last().is_some_and(|t| fillers.contains(t)) {
        toks.pop();
    }
    toks.join(" ")
}

fn has_meet_word(body: &str) -> bool {
    body.split_whitespace().any(|w| matches!(w, "meet" | "mate" | "meat" | "mete"))
}

fn extract_join_contact(body: &str) -> String {
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

fn is_join_request(body: &str) -> bool {
    if !has_meet_word(body) {
        return false;
    }
    let head = body.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
    body.starts_with("join ") || head.starts_with("join ")
}

fn utterance_is_meet_shaped(body: &str) -> bool {
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.len() <= 14 {
        return true;
    }
    let head = words[..8.min(words.len())].join(" ");
    head.starts_with("make ")
        || head.starts_with("create ")
        || head.starts_with("start ")
        || head.starts_with("open ")
        || head.starts_with("send ")
        || head.starts_with("join ")
        || head.contains("google meet")
        || head.contains("google mate")
        || head.contains("google meat")
        || head.contains("google mete")
}

fn strip_leading_chatter(body: &str) -> String {
    const CHATTER: &[&str] = &[
        "hello", "hi", "hey", "yo", "sup", "kuro", "please", "now", "can", "you", "could", "would", "just", "kindly",
        "actually", "really", "okay", "ok", "alright", "so", "well", "umm", "uh", "cande",
    ];
    const ACTIONS: &[&str] = &["send", "make", "create", "start", "join", "open"];
    let mut toks: Vec<&str> = body.split_whitespace().collect();
    while let Some(first) = toks.first().copied() {
        if ACTIONS.contains(&first) {
            break;
        }
        if CHATTER.contains(&first) {
            toks.remove(0);
            continue;
        }
        break;
    }
    toks.join(" ")
}

fn soft_send_to_contact(body: &str) -> Option<HashMap<String, String>> {
    if !has_meet_word(body) {
        return None;
    }
    let createish = body.starts_with("send ")
        || body.starts_with("make ")
        || body.starts_with("create ")
        || body.starts_with("start ")
        || body.contains(" send ");
    if !createish || is_join_request(body) {
        return None;
    }
    let book = crate::whatsapp::address_book();
    if book.is_empty() {
        return None;
    }
    let toks: Vec<&str> = body.split_whitespace().collect();
    for n in 1..=3 {
        if toks.len() < n {
            break;
        }
        let candidate = clean_name(&toks[toks.len() - n..].join(" "));
        if candidate.is_empty() || has_meet_word(&candidate) {
            continue;
        }
        if crate::contact_bias::resolve_contact(&candidate, &book).is_some() {
            let mut out = HashMap::new();
            out.insert("action".into(), "create".into());
            out.insert("contact".into(), candidate);
            return Some(out);
        }
    }
    None
}

/// True when the user clearly asked for Meet actuation (tool path, not chat waffle).
pub fn looks_like_meet_request(text: &str) -> bool {
    let body = strip_leading_chatter(&normalize(text));
    if !has_meet_word(&body) {
        return false;
    }
    body.starts_with("send ")
        || body.starts_with("make ")
        || body.starts_with("create ")
        || body.starts_with("start ")
        || body.starts_with("join ")
        || body.starts_with("open ")
        || body.contains("google meet")
        || body.contains("google mate")
}

pub fn match_phrase(text: &str) -> Option<HashMap<String, String>> {
    let body = strip_leading_chatter(&normalize(text));
    if !utterance_is_meet_shaped(&body) {
        return None;
    }
    if is_join_request(&body) {
        let mut out = HashMap::new();
        out.insert("action".into(), "join".into());
        out.insert("contact".into(), extract_join_contact(&body));
        return Some(out);
    }
    if meet_followup::match_followup(text).is_some() {
        return None;
    }
    let re = Regex::new(
        r"^(?:(?:make|create|start)\s+(?:a\s+|an\s+)?(?:google\s+)?(?:meet|mate|meat|mete)(?:\s+link)?(?:\s+and\s+send(?:\s+(?:the|this|a))?(?:\s+link)?(?:\s+it)?\s+(?:to|td|too|for)\s+(?P<to>.+)|\s+for\s+(?P<forname>.+))?|send\s+(?:me\s+)?(?:the\s+|a\s+|my\s+)?(?:google\s+)?(?:meet|mate|meat|mete)(?:\s+link)?\s+(?:to|td|too|for)\s+(?P<sendto>.+))$",
    )
    .ok()?;
    if let Some(caps) = re.captures(&body).ok().flatten() {
        let raw = caps
            .name("to")
            .or_else(|| caps.name("forname"))
            .or_else(|| caps.name("sendto"))
            .map(|m| m.as_str())
            .unwrap_or("");
        let contact = clean_name(raw);
        if !contact.is_empty() && has_meet_word(&contact) {
            return None;
        }
        let mut out = HashMap::new();
        out.insert("action".into(), "create".into());
        out.insert("contact".into(), contact);
        return Some(out);
    }
    soft_send_to_contact(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn td_typo_matches_send_to_siya() {
        crate::whatsapp::set_address_book(Some([("Siya".into(), "910000000001".into())].into_iter().collect()));
        let found = match_phrase("Send a Google Meet link TD Siya.").unwrap();
        assert_eq!(found.get("action").map(String::as_str), Some("create"));
        assert_eq!(found.get("contact").map(String::as_str), Some("siya"));
        crate::whatsapp::set_address_book(None);
    }

    #[test]
    fn garbage_prefix_still_matches() {
        crate::whatsapp::set_address_book(Some([("Siya".into(), "910000000001".into())].into_iter().collect()));
        let found = match_phrase("~CANDE you send a Google Meet link TD Siya for PAS?").unwrap();
        assert_eq!(found.get("contact").map(String::as_str), Some("siya"));
        crate::whatsapp::set_address_book(None);
    }
}
