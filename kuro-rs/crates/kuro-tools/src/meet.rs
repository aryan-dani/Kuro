use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fancy_regex::Regex;
use serde_json::Value;

thread_local! {
    static CREDS: RefCell<Option<bool>> = const { RefCell::new(None) };
}

static MEET_BUSY: AtomicBool = AtomicBool::new(false);
/// Unix seconds until a new Meet may start (cooldown after a claim).
static MEET_COOLDOWN_UNTIL: AtomicU64 = AtomicU64::new(0);
const MEET_COOLDOWN_SECS: u64 = 45;

/// `Some(false)` forces "not signed in" without reading a file. `None` uses live credentials.
pub fn set_signed_in(value: Option<bool>) {
    CREDS.with(|s| *s.borrow_mut() = value);
}

pub fn edge_args(edge: &str, link: &str) -> Vec<String> {
    vec![edge.to_string(), format!("--app={link}"), "--start-maximized".into()]
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn claim_meet() -> bool {
    if MEET_BUSY.swap(true, Ordering::SeqCst) {
        return false;
    }
    let now = unix_now();
    let until = MEET_COOLDOWN_UNTIL.load(Ordering::SeqCst);
    if now < until {
        MEET_BUSY.store(false, Ordering::SeqCst);
        return false;
    }
    true
}

fn release_meet(apply_cooldown: bool) {
    if apply_cooldown {
        MEET_COOLDOWN_UNTIL.store(unix_now().saturating_add(MEET_COOLDOWN_SECS), Ordering::SeqCst);
    }
    MEET_BUSY.store(false, Ordering::SeqCst);
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
    // Intentionally omit "meeting" — it matches "meeting notes" and other chat.
    body.split_whitespace().any(|w| matches!(w, "meet" | "mate" | "meat" | "mete"))
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

/// Long STT lines that merely mention Meet must not open a call.
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
        || head.contains("google meet")
        || head.contains("google mate")
        || head.contains("google meat")
        || head.contains("google mete")
}

pub fn match_phrase(text: &str) -> Option<std::collections::HashMap<String, String>> {
    let body = normalize(text);
    if !utterance_is_meet_shaped(&body) {
        return None;
    }
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

fn edge_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program = std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles")).map(PathBuf::from);
    let mut candidates = Vec::new();
    if let Some(p) = program {
        candidates.push(p.join(r"Microsoft\Edge\Application\msedge.exe"));
    }
    if let Some(p) = local {
        candidates.push(p.join(r"Microsoft\Edge\Application\msedge.exe"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn open_edge(link: &str) -> bool {
    let Some(edge) = edge_path() else {
        return false;
    };
    let args = edge_args(edge.to_string_lossy().as_ref(), link);
    Command::new(&args[0]).args(&args[1..]).spawn().is_ok()
}

fn access_token(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    if !refresh_if_needed(path, &data) {
        // Still try whatever token is on disk.
    }
    let text = std::fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    let token = data.get("token").or_else(|| data.get("access_token")).and_then(|v| v.as_str()).unwrap_or("").trim();
    if token.is_empty() { None } else { Some(token.to_string()) }
}

fn refresh_if_needed(path: &Path, data: &Value) -> bool {
    let refresh = data.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("");
    if refresh.is_empty() || !token_expired(data) {
        return true;
    }
    refresh_token(path, data, refresh)
}

pub fn run(arguments: &Value) -> String {
    let contact = arguments.get("contact").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let mut digits = String::new();
    let mut shown = String::new();
    if !contact.is_empty() {
        let book = crate::whatsapp::address_book();
        let Some((name, number)) = crate::whatsapp::resolve_contact(&contact, &book) else {
            return "I don't know that contact.".into();
        };
        digits = number;
        shown = crate::whatsapp::spoken_recipient(&name);
    }
    match CREDS.with(|s| *s.borrow()) {
        Some(false) => return "Google Meet is not signed in.".into(),
        Some(true) => return "Success: The Meet is open.".into(),
        None => {}
    }
    let token_path = kuro_core::KuroPaths::from_env().google_token();
    if !google_token_usable(&token_path) {
        return "Google Meet is not signed in.".into();
    }
    if !claim_meet() {
        return "I'm already opening a Meet.".into();
    }
    let result = run_native(&token_path, &digits, &shown);
    let cool = result.starts_with("Success:") || result.contains("Sent the Meet");
    release_meet(cool);
    result
}

fn run_native(token_path: &Path, digits: &str, shown: &str) -> String {
    let Some(token) = access_token(token_path) else {
        return "Google Meet is not signed in.".into();
    };
    let Some(link) = create_space(&token) else {
        return "I couldn't create that Google Meet.".into();
    };

    // Link first so the contact is not waiting on Edge.
    let sent_ok = if digits.is_empty() {
        true
    } else {
        match crate::whatsapp::deliver_text(digits, &link) {
            Ok(()) => true,
            Err(err) => {
                let _ = open_edge(&link);
                return err;
            }
        }
    };

    if !open_edge(&link) {
        if !digits.is_empty() && sent_ok {
            return format!("Success: Sent the Meet link to {shown}, but Edge did not open.");
        }
        return "The Meet was created, but Edge did not open.".into();
    }
    if digits.is_empty() {
        return "Success: The Meet is open.".into();
    }
    format!("Success: Sent the Meet link to {shown}.")
}

fn google_token_usable(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    let token = data.get("token").or_else(|| data.get("access_token")).and_then(|v| v.as_str()).unwrap_or("");
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
    // Google token.json uses RFC3339; compare lexicographically only for ISO-like stamps.
    if expiry.contains('T') {
        return chrono_rfc_expired(expiry);
    }
    expiry.len() >= 10 && expiry < chrono_now().as_str()
}

fn chrono_rfc_expired(expiry: &str) -> bool {
    // Prefer refresh when expiry string is in the past vs UTC now (ISO-8601 comparable).
    expiry < chrono_iso_now().as_str()
}

fn chrono_now() -> String {
    let secs = unix_now();
    format!("{secs}")
}

fn chrono_iso_now() -> String {
    // Enough for string compare against Google's `2025-01-02T03:04:05Z` style expiry.
    let secs = unix_now() as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let hours = rem / 3600;
    let mins = (rem % 3600) / 60;
    let s = rem % 60;
    // Civil date from Unix day (proleptic Gregorian).
    let (y, m, d) = unix_day_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{mins:02}:{s:02}Z")
}

fn unix_day_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm from civil_from_days (Howard Hinnant).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
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
        // Google returns access_token; keep the authorized-user `token` field in sync.
        if let Some(access) = out.get("access_token").cloned() {
            out.insert("token".into(), access);
        }
    }
    let _ = std::fs::write(path, serde_json::to_string_pretty(&fresh).unwrap_or_default());
    fresh.get("access_token").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty())
        || fresh.get("token").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty())
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
