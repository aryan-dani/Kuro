use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

fn has_meet_word(body: &str) -> bool {
    body.split_whitespace().any(|w| matches!(w, "meet" | "mate" | "meat" | "mete" | "meeting"))
}

fn extract_send_to(body: &str) -> Option<String> {
    for marker in ["send it to ", "send the link to ", "send link to ", "send to ", " to "] {
        if let Some(idx) = body.rfind(marker) {
            let rest = body[idx + marker.len()..].trim();
            if rest.is_empty() {
                continue;
            }
            let name = clean_name(rest);
            if !name.is_empty() && !has_meet_word(&name) {
                return Some(name);
            }
        }
    }
    None
}

pub fn match_phrase(text: &str) -> Option<std::collections::HashMap<String, String>> {
    let body = normalize(text);
    let re = Regex::new(
        r"^(?:(?:make|create|start)\s+(?:a\s+|an\s+)?(?:google\s+)?(?:meet|mate|meat|mete)(?:\s+and\s+send(?:\s+(?:the|this|a))?(?:\s+link)?(?:\s+it)?\s+to\s+(?P<to>.+)|\s+for\s+(?P<forname>.+))?|send\s+(?:me\s+)?(?:the\s+|a\s+|my\s+)?(?:google\s+)?(?:meet|mate|meat|mete)(?:\s+link)?\s+to\s+(?P<sendto>.+))$",
    )
    .ok()?;
    if let Some(caps) = re.captures(&body).ok().flatten() {
        let raw = caps
            .name("to")
            .or_else(|| caps.name("forname"))
            .or_else(|| caps.name("sendto"))
            .map(|m| m.as_str())
            .unwrap_or("");
        let mut out = std::collections::HashMap::new();
        out.insert("contact".into(), clean_name(raw));
        return Some(out);
    }
    // Looser voice lines: "google meet link for mom", "send meet to mom on whatsapp", etc.
    if !has_meet_word(&body) {
        return None;
    }
    let contact = extract_send_to(&body).unwrap_or_default();
    if contact.is_empty()
        && !body.contains("make")
        && !body.contains("create")
        && !body.contains("start")
        && !body.contains("send")
        && !body.contains("open")
    {
        return None;
    }
    let mut out = std::collections::HashMap::new();
    out.insert("contact".into(), contact);
    Some(out)
}

fn repo_root() -> PathBuf {
    if let Ok(root) = std::env::var("KURO_ROOT") {
        let path = PathBuf::from(root.trim());
        if path.is_dir() {
            return path;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..")
}

fn python_exe(root: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("KURO_PYTHON") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return path;
        }
    }
    let venv = root.join(".venv").join("Scripts").join("python.exe");
    if venv.is_file() {
        return venv;
    }
    PathBuf::from("python")
}

fn spawn_meet_py(contact: &str) -> String {
    let root = repo_root();
    let python = python_exe(&root);
    let script = r#"
import json, sys
payload = json.loads(sys.stdin.read())
from tools.meet import run
print(run(payload))
"#;
    let payload = serde_json::json!({ "contact": contact }).to_string();
    let mut child = match Command::new(&python)
        .arg("-c")
        .arg(script)
        .current_dir(&root)
        .env("PYTHONPATH", &root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return "I couldn't create that Google Meet.".into(),
    };
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(payload.as_bytes()).is_err()
    {
        return "I couldn't create that Google Meet.".into();
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(_) => return "I couldn't create that Google Meet.".into(),
    };
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { "I couldn't create that Google Meet.".into() } else { text }
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
    spawn_meet_py(&contact)
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
    if token.is_empty() && refresh.is_empty() {
        return false;
    }
    if !refresh.is_empty() && token_expired(&data) {
        return refresh_token(path, &data, refresh);
    }
    !token.is_empty() || !refresh.is_empty()
}

fn token_expired(data: &Value) -> bool {
    let Some(expiry) = data.get("expiry").and_then(|v| v.as_str()) else {
        return false;
    };
    expiry.len() >= 10 && expiry < chrono_now().as_str()
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{secs}")
}

fn refresh_token(path: &Path, data: &Value, refresh: &str) -> bool {
    let client_id = data.get("client_id").and_then(|v| v.as_str()).unwrap_or("");
    let client_secret = data.get("client_secret").and_then(|v| v.as_str()).unwrap_or("");
    if client_id.is_empty() {
        return false;
    }
    let form = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        urlencoding_min(client_id),
        urlencoding_min(client_secret),
        urlencoding_min(refresh)
    );
    let resp = ureq::post("https://oauth2.googleapis.com/token")
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form);
    let Ok(resp) = resp else {
        return false;
    };
    let Ok(text) = resp.into_string() else {
        return false;
    };
    let Ok(mut fresh) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    if let Some(obj) = data.as_object()
        && let Some(out) = fresh.as_object_mut()
    {
        for (k, v) in obj {
            out.entry(k.clone()).or_insert(v.clone());
        }
    }
    let _ = std::fs::write(path, serde_json::to_string_pretty(&fresh).unwrap_or_default());
    fresh.get("access_token").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty())
}

fn urlencoding_min(raw: &str) -> String {
    let mut out = String::new();
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
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
