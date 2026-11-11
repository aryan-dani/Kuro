use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use fancy_regex::Regex;
use kuro_core::get_close_matches;
use serde_json::Value;

const CUTOFF: f64 = 0.75;
const FILLERS: &[&str] = &["please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"];
const IMAGE_FORMATS: &[i32] = &[2, 8, 17, 15];

thread_local! {
    static CONTACTS: RefCell<Option<HashMap<String, String>>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
    static BOOK: RefCell<Option<HashMap<String, String>>> = const { RefCell::new(None) };
}

pub fn set_contacts(book: Option<HashMap<String, String>>) {
    CONTACTS.with(|s| *s.borrow_mut() = book);
}

pub fn set_session_path(path: Option<std::path::PathBuf>) {
    SESSION.with(|s| *s.borrow_mut() = path);
}

pub fn set_address_book(book: Option<HashMap<String, String>>) {
    BOOK.with(|s| *s.borrow_mut() = book);
}

pub fn normalize(text: &str) -> String {
    crate::n8n::normalize(text)
}

pub fn clean_digits(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn url_ok(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !value.split_whitespace().nth(1).is_some()
        && value.len() > if lower.starts_with("https://") { 8 } else { 7 }
}

pub fn spoken_recipient(name: &str) -> String {
    if normalize(name) == "me" {
        return "you".into();
    }
    let cleaned = name.trim();
    if cleaned.is_empty() {
        return "them".into();
    }
    let mut chars = cleaned.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    format!("{first}{}", chars.as_str())
}

pub fn target_is_front(foreground: i64, target: i64) -> bool {
    target != 0 && foreground == target
}

pub fn is_whatsapp_title(title: &str) -> bool {
    let text = title.trim().to_ascii_lowercase();
    if text.is_empty() || ["chrome", "edge", "firefox", "http"].iter().any(|s| text.contains(s)) {
        return false;
    }
    text == "whatsapp" || text.ends_with(") whatsapp")
}

pub fn payload_kind(text: &str, formats: &HashSet<i32>) -> &'static str {
    if !text.trim().is_empty() {
        return "text";
    }
    if formats.iter().any(|f| IMAGE_FORMATS.contains(f)) {
        return "image";
    }
    "empty"
}

pub fn load_contacts() -> HashMap<String, String> {
    if let Some(book) = CONTACTS.with(|s| s.borrow().clone()) {
        return book;
    }
    let path = kuro_core::KuroPaths::from_env().contacts();
    read_contacts(&path)
}

fn read_contacts(path: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return HashMap::new();
    };
    let Some(obj) = data.as_object() else {
        return HashMap::new();
    };
    let mut book = HashMap::new();
    for (name, number) in obj {
        let digits = clean_digits(number.as_str().unwrap_or(""));
        if !name.trim().is_empty() && digits.len() >= 8 {
            book.insert(name.trim().to_string(), digits);
        }
    }
    book
}

fn jid_digits(jid: &str) -> String {
    let raw = jid.trim().to_ascii_lowercase();
    let Some((user, host)) = raw.split_once('@') else {
        return String::new();
    };
    if !host.starts_with("s.whatsapp.net") {
        return String::new();
    }
    let user = user.split(':').next().unwrap_or("");
    let digits = clean_digits(user);
    if digits.len() < 8 || digits != user {
        return String::new();
    }
    digits
}

pub fn load_wa_contacts(path: &Path) -> HashMap<String, String> {
    if !path.is_file() {
        return HashMap::new();
    }
    let Ok(con) = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return HashMap::new();
    };
    let mut stmt = match con
        .prepare("SELECT first_name, full_name, push_name, business_name, their_jid FROM whatsmeow_contacts")
    {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0).unwrap_or_default(),
            row.get::<_, String>(1).unwrap_or_default(),
            row.get::<_, String>(2).unwrap_or_default(),
            row.get::<_, String>(3).unwrap_or_default(),
            row.get::<_, String>(4).unwrap_or_default(),
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return HashMap::new(),
    };
    let mut grouped: HashMap<String, (String, HashSet<String>)> = HashMap::new();
    for row in rows.flatten() {
        let digits = jid_digits(&row.4);
        if digits.is_empty() {
            continue;
        }
        for raw in [&row.1, &row.2, &row.0, &row.3] {
            let name = raw.trim();
            let key = normalize(name);
            if key.is_empty() {
                continue;
            }
            let entry = grouped.entry(key).or_insert_with(|| (name.to_string(), HashSet::new()));
            entry.1.insert(digits.clone());
        }
    }
    let mut book = HashMap::new();
    for (_key, (display, numbers)) in grouped {
        if numbers.len() == 1 {
            book.insert(display, numbers.into_iter().next().unwrap());
        }
    }
    book
}

pub fn address_book() -> HashMap<String, String> {
    if let Some(book) = BOOK.with(|s| s.borrow().clone()) {
        return book;
    }
    let mut book = load_contacts();
    let mut taken: HashSet<String> = book.keys().map(|n| normalize(n)).collect();
    let session = SESSION.with(|s| s.borrow().clone()).unwrap_or_else(|| kuro_core::KuroPaths::from_env().wa_session());
    for (name, digits) in load_wa_contacts(&session) {
        let key = normalize(&name);
        if !key.is_empty() && !taken.contains(&key) {
            taken.insert(key);
            book.insert(name, digits);
        }
    }
    book
}

fn edit_distance(a: &str, b: &str) -> usize {
    if a.len().abs_diff(b.len()) > 1 {
        return 2;
    }
    if a.len() == b.len() {
        return a.bytes().zip(b.bytes()).filter(|(x, y)| x != y).count();
    }
    let (longer, shorter) = if a.len() > b.len() { (a, b) } else { (b, a) };
    let lb = longer.as_bytes();
    let sb = shorter.as_bytes();
    let mut i = 0;
    let mut j = 0;
    let mut skipped = 0;
    while i < lb.len() && j < sb.len() {
        if lb[i] != sb[j] {
            skipped += 1;
            if skipped > 1 {
                return 2;
            }
            i += 1;
            continue;
        }
        i += 1;
        j += 1;
    }
    1
}

fn shared_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

fn one_letter_miss(query_n: &str, book: &HashMap<String, String>) -> Option<(String, String)> {
    let mut found: HashMap<String, (String, String)> = HashMap::new();
    for (name, digits) in book {
        let key = normalize(name);
        if !key.is_empty() && edit_distance(query_n, &key) == 1 {
            found.entry(key).or_insert_with(|| (name.clone(), digits.clone()));
        }
    }
    if found.is_empty() {
        return None;
    }
    if found.len() == 1 {
        return found.into_values().next();
    }
    let mut ranked: Vec<(String, (String, String))> = found.into_iter().collect();
    ranked.sort_by_key(|pair| std::cmp::Reverse(shared_prefix(query_n, &pair.0)));
    let best_n = shared_prefix(query_n, &ranked[0].0);
    let next_n = shared_prefix(query_n, &ranked[1].0);
    if best_n >= 2 && best_n > next_n {
        return Some(ranked.remove(0).1);
    }
    None
}

pub fn resolve_contact(query: &str, book: &HashMap<String, String>) -> Option<(String, String)> {
    let query_n = normalize(query);
    if query_n.is_empty() || book.is_empty() {
        return None;
    }
    for (name, digits) in book {
        if normalize(name) == query_n {
            return Some((name.clone(), digits.clone()));
        }
    }
    let choices: Vec<&str> = book.keys().map(|s| s.as_str()).collect();
    if let Some(hit) = get_close_matches(query, &choices, 1, CUTOFF).into_iter().next() {
        return book.get(hit).map(|d| (hit.to_string(), d.clone()));
    }
    let norms: Vec<String> = choices.iter().map(|n| normalize(n)).collect();
    let refs: Vec<&str> = norms.iter().map(|s| s.as_str()).collect();
    if let Some(hit) = get_close_matches(&query_n, &refs, 1, CUTOFF).into_iter().next() {
        for (name, digits) in book {
            if normalize(name) == hit {
                return Some((name.clone(), digits.clone()));
            }
        }
    }
    one_letter_miss(&query_n, book)
}

fn strip_fillers(text: &str) -> String {
    let norm = normalize(text);
    let mut toks: Vec<&str> = norm.split_whitespace().collect();
    while toks.first().is_some_and(|t| FILLERS.contains(t)) {
        toks.remove(0);
    }
    toks.join(" ")
}

fn source_for(kind: &str) -> &'static str {
    if kind == "link" || kind == "meeting link" { "browser_url" } else { "clipboard" }
}

pub fn match_phrase(text: &str) -> Option<HashMap<String, String>> {
    let body = strip_fillers(text);
    let re = Regex::new(
        r"^(?:send\s+(?:me\s+)?(?:(?:my|the|this)\s+)?(?P<kind>meeting link|link|clipboard|message)\s+to\s+(?P<name>.+?)\s+(?:on|in|via)\s+whatsapp|send\s+(?:me\s+)?(?:(?:my|the|this)\s+)?(?P<kind_me>meeting link|link|clipboard|message)(?:\s+(?!to\b)\w+){0,4}\s+(?:on|in|via)\s+whatsapp|whatsapp\s+(?P<name2>.+?)\s+(?:this|the)\s+link)$",
    )
    .ok()?;
    let caps = re.captures(&body).ok().flatten()?;
    let kind_me = caps.name("kind_me").map(|m| m.as_str().to_string());
    if let Some(kind) = kind_me {
        let mut out = HashMap::new();
        out.insert("contact".into(), "me".into());
        out.insert("source".into(), source_for(&kind).into());
        return Some(out);
    }
    let name =
        caps.name("name").or_else(|| caps.name("name2")).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    let source = if let Some(kind) = caps.name("kind") { source_for(kind.as_str()) } else { "browser_url" };
    let mut out = HashMap::new();
    out.insert("contact".into(), name);
    out.insert("source".into(), source.into());
    Some(out)
}

/// Phrase and contact resolution stay in-process. The neonize send is a one-shot process.
pub fn run(arguments: &Value) -> String {
    let contact = arguments.get("contact").and_then(|v| v.as_str()).unwrap_or("").trim();
    let source = arguments.get("source").and_then(|v| v.as_str()).unwrap_or("").trim().to_ascii_lowercase();
    if !["clipboard", "browser_url", "text"].contains(&source.as_str()) {
        return "I don't know how to send that.".into();
    }
    let book = address_book();
    let Some((name, digits)) = resolve_contact(contact, &book) else {
        return "I don't know who to message.".into();
    };
    let text = arguments.get("text").and_then(|v| v.as_str()).unwrap_or("");
    match spawn_neonize(&digits, &source, text) {
        Ok(()) => format!("Success: Sent that to {}.", spoken_recipient(&name)),
        Err(err) => err,
    }
}

fn spawn_neonize(digits: &str, source: &str, text: &str) -> Result<(), String> {
    let python = std::env::var("KURO_PYTHON").unwrap_or_else(|_| "python".into());
    let script = r#"
import json, os, sys
payload = json.loads(sys.stdin.read())
try:
    from tools.whatsapp import deliver
except Exception as exc:
    print("missing", file=sys.stderr)
    sys.exit(3)
result = deliver(payload["digits"], text=payload.get("text") or "", image=None)
print(result)
sys.exit(0 if result == "ok" else 2)
"#;
    let payload = serde_json::json!({
        "digits": digits,
        "source": source,
        "text": text,
    })
    .to_string();
    let mut child = std::process::Command::new(&python)
        .arg("-c")
        .arg(script)
        .env("PYTHONPATH", std::env::var("KURO_ROOT").unwrap_or_else(|_| ".".into()))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().ok_or_else(|| "I couldn't send that WhatsApp.".to_string())?;
        stdin.write_all(payload.as_bytes()).map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
    }
    let output = child.wait_with_output().map_err(|_| "I couldn't send that WhatsApp.".to_string())?;
    if output.status.success() { Ok(()) } else { Err("I couldn't send that WhatsApp.".into()) }
}
