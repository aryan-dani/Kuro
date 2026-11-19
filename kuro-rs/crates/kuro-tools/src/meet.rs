use crate::meet_busy::claim_meet;
use crate::meet_http::{access_token, google_token_usable};
use fancy_regex::Regex;
use serde_json::Value;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;

thread_local! {
    static CREDS: RefCell<Option<bool>> = const { RefCell::new(None) };
}

/// `Some(false)` forces "not signed in" without reading a file. `None` uses live credentials.
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
    // Intentionally omit "meeting"; it matches "meeting notes" and other chat.
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

fn meet_open_enabled() -> bool {
    matches!(
        std::env::var("KURO_MEET_OPEN").unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn open_edge(link: &str) -> bool {
    if !meet_open_enabled() {
        // Default off: creating/sending a Meet must not steal focus with a new Edge tab.
        return false;
    }
    let Some(edge) = edge_path() else {
        return false;
    };
    let args = edge_args(edge.to_string_lossy().as_ref(), link);
    Command::new(&args[0]).args(&args[1..]).spawn().is_ok()
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
    let Some(guard) = claim_meet() else {
        return "I'm already opening a Meet.".into();
    };
    let result = run_native(&token_path, &digits, &shown, &MeetHttp::production());
    let cool = result.starts_with("Success:") || result.contains("Sent the Meet");
    if cool {
        guard.mark_cooldown();
    }
    result
}

fn run_native(token_path: &Path, digits: &str, shown: &str, http: &MeetHttp) -> String {
    let Some(token) = access_token(token_path, http) else {
        return "Google Meet is not signed in.".into();
    };
    let Some(link) = create_space(&token, http) else {
        return "I couldn't create that Google Meet.".into();
    };

    // Link first so the contact is not waiting on a browser.
    let sent_ok = if digits.is_empty() {
        true
    } else {
        match crate::whatsapp::deliver_text(digits, &link) {
            Ok(()) => true,
            Err(err) => return err,
        }
    };

    let opened = open_edge(&link);
    if digits.is_empty() {
        if opened {
            return "Success: The Meet is open.".into();
        }
        // Default: create only; speak the fact without forcing a tab.
        return format!("Success: Meet ready. {link}");
    }
    if sent_ok && opened {
        return format!("Success: Sent the Meet link to {shown}.");
    }
    if sent_ok {
        return format!("Success: Sent the Meet link to {shown}.");
    }
    "I couldn't send that Meet link.".into()
}

pub use crate::meet_http::{MeetHttp, create_space};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meet_busy::{MEET_BUSY, claim_meet};
    use crate::meet_http::token_expired;
    use std::io::Read;
    use std::net::TcpListener;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    #[test]
    fn busy_released_on_panic() {
        MEET_BUSY.store(false, Ordering::SeqCst);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = claim_meet().expect("claimed");
            panic!("meet worker");
        }));
        assert!(!MEET_BUSY.load(Ordering::SeqCst));
    }

    #[test]
    fn expiry_uses_real_time() {
        use chrono::{Duration as ChronoDuration, Utc};
        use serde_json::json;

        let future = (Utc::now() + ChronoDuration::hours(2)).to_rfc3339();
        assert!(!token_expired(&json!({"expiry": future, "token": "t"})));

        let past = (Utc::now() - ChronoDuration::hours(2)).to_rfc3339();
        assert!(token_expired(&json!({"expiry": past, "token": "t"})));

        let inside_skew = (Utc::now() - ChronoDuration::seconds(30)).to_rfc3339();
        assert!(!token_expired(&json!({"expiry": inside_skew, "token": "t"})));

        let offset = (Utc::now() + ChronoDuration::hours(1))
            .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
            .to_rfc3339();
        assert!(!token_expired(&json!({"expiry": offset, "token": "t"})));

        assert!(token_expired(&json!({"expiry": "not-a-date", "token": "t"})));
        assert!(token_expired(&json!({"token": "t"})));
    }

    #[test]
    fn meet_hung_api_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let url = format!("http://{addr}/v2/spaces");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0_u8; 1024];
                let _ = stream.read(&mut buf);
                std::thread::sleep(Duration::from_secs(30));
            }
        });
        let http = MeetHttp { token_url: url.clone(), spaces_url: url };
        let start = Instant::now();
        assert!(create_space("test-token", &http).is_none());
        assert!(start.elapsed() < Duration::from_secs(9));
    }
}
