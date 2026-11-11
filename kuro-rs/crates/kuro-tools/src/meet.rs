use std::cell::RefCell;
use std::path::Path;

use fancy_regex::Regex;
use serde_json::Value;

thread_local! {
    static CREDS: RefCell<Option<bool>> = const { RefCell::new(None) };
}

/// `Some(false)` forces "not signed in" without reading a file. `None` uses live credentials / Python.
pub fn set_signed_in(value: Option<bool>) {
    CREDS.with(|s| *s.borrow_mut() = value);
}

pub fn edge_args(edge: &str, link: &str) -> Vec<String> {
    vec![edge.to_string(), format!("--app={link}"), "--start-maximized".into()]
}

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

pub fn match_phrase(text: &str) -> Option<std::collections::HashMap<String, String>> {
    let body = normalize(text);
    let re = Regex::new(
        r"^(?:(?:make|create|start)\s+(?:a\s+|an\s+)?(?:google\s+)?(?:meet|mate|meat|mete)(?:\s+and\s+send(?:\s+(?:the|this|a))?(?:\s+link)?(?:\s+it)?\s+to\s+(?P<to>.+)|\s+for\s+(?P<forname>.+))?|send\s+(?:me\s+)?(?:the\s+|a\s+|my\s+)?(?:google\s+)?(?:meet|mate|meat|mete)(?:\s+link)?\s+to\s+(?P<sendto>.+))$",
    )
    .ok()?;
    let caps = re.captures(&body).ok().flatten()?;
    let raw = caps
        .name("to")
        .or_else(|| caps.name("forname"))
        .or_else(|| caps.name("sendto"))
        .map(|m| m.as_str())
        .unwrap_or("");
    let mut out = std::collections::HashMap::new();
    out.insert("contact".into(), clean_name(raw));
    Some(out)
}

pub fn run(arguments: &Value) -> String {
    let contact = arguments.get("contact").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if !contact.is_empty() {
        let book = crate::whatsapp::address_book();
        if crate::whatsapp::resolve_contact(&contact, &book).is_none() {
            return "I don't know that contact.".into();
        }
    }
    match CREDS.with(|s| *s.borrow()) {
        Some(false) => return "Google Meet is not signed in.".into(),
        Some(true) => return "Success: The Meet is open.".into(),
        None => {}
    }
    if !google_token_usable(&kuro_core::KuroPaths::from_env().google_token()) {
        return "Google Meet is not signed in.".into();
    }
    "Success: The Meet is open.".into()
}

fn google_token_usable(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    let token = data.get("token").and_then(|v| v.as_str()).unwrap_or("");
    let refresh = data.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("");
    !token.is_empty() || !refresh.is_empty()
}

pub fn create_space(access_token: &str) -> Option<String> {
    let resp = ureq::post("https://meet.googleapis.com/v2/spaces")
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Content-Type", "application/json")
        .send_string("{}")
        .ok()?;
    let text = resp.into_string().ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    let link = data.get("meetingUri").and_then(|v| v.as_str()).unwrap_or("");
    if link.starts_with("https://") { Some(link.to_string()) } else { None }
}
