use crate::meet_busy::claim_meet;
use crate::meet_http::{access_token, google_token_usable};
use crate::meet_last;
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

/// Engine boot must call this once. Tests and gate leave it off so Meet stays dry.
pub fn enable_live_meet() {
    crate::meet_busy::enable_live_meet();
}

pub use crate::meet_match::{looks_like_meet_request, match_phrase};

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

/// Open Edge unless `KURO_MEET_OPEN` is explicitly off.
fn meet_open_enabled() -> bool {
    match std::env::var("KURO_MEET_OPEN") {
        Ok(v) => !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"),
        Err(_) => true,
    }
}

fn open_edge(link: &str) -> Result<bool, &'static str> {
    if !crate::meet_busy::live_meet_enabled() || !meet_open_enabled() {
        return Ok(false);
    }
    let Some(edge) = edge_path() else {
        return Err("missing");
    };
    let args = edge_args(edge.to_string_lossy().as_ref(), link);
    if Command::new(&args[0]).args(&args[1..]).spawn().is_ok() { Ok(true) } else { Err("spawn") }
}

pub fn run(arguments: &Value) -> String {
    let contact = arguments.get("contact").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    kuro_core::append_rotating_log(
        &kuro_core::KuroPaths::from_env().log(),
        &format!("make_meet start contact=`{contact}` live={}", crate::meet_busy::live_meet_enabled()),
        1_000_000,
    );
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
    if !crate::meet_busy::live_meet_enabled() {
        return "Google Meet is not signed in.".into();
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

/// Open a previously created Meet link. Does not create a space or take create cooldown.
/// Edge spawn still requires `enable_live_meet` (engine boot).
pub fn run_join(arguments: &Value) -> String {
    let contact = arguments.get("contact").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let path = meet_last::store_path();
    if !contact.is_empty() {
        let book = crate::whatsapp::address_book();
        let Some((name, _)) = crate::whatsapp::resolve_contact(&contact, &book) else {
            return "I don't know that contact.".into();
        };
        let shown = crate::whatsapp::spoken_recipient(&name);
        let Some(link) = meet_last::lookup(&path, &name).or_else(|| meet_last::lookup(&path, &contact)) else {
            return "I don't have a Meet link for them.".into();
        };
        return open_saved_link(&link, Some(&shown));
    }
    let Some(link) = meet_last::lookup(&path, "") else {
        return "I don't have a Meet link for them.".into();
    };
    open_saved_link(&link, None)
}

fn open_saved_link(link: &str, _shown: Option<&str>) -> String {
    match open_edge(link) {
        Ok(true) => "Success: The Meet is open.".into(),
        Ok(false) => format!("Success: Meet ready. {link}"),
        Err("missing") => "I couldn't find Microsoft Edge.".into(),
        Err(_) => "I couldn't open that Meet link.".into(),
    }
}

fn run_native(token_path: &Path, digits: &str, shown: &str, http: &MeetHttp) -> String {
    let Some(token) = access_token(token_path, http) else {
        return "Google Meet is not signed in.".into();
    };
    let Some(link) = create_space(&token, http) else {
        return "I couldn't create that Google Meet.".into();
    };

    meet_last::remember(&meet_last::store_path(), &link, shown);

    // Link first so the contact is not waiting on a browser.
    let sent_ok = if digits.is_empty() {
        true
    } else {
        match crate::whatsapp::deliver_text(digits, &link) {
            Ok(()) => true,
            Err(err) => return err,
        }
    };

    let opened = match open_edge(&link) {
        Ok(v) => v,
        Err("missing") if digits.is_empty() => {
            return "I couldn't find Microsoft Edge.".into();
        }
        Err(_) => false,
    };
    if digits.is_empty() {
        if opened {
            return "Success: The Meet is open.".into();
        }
        return format!("Success: Meet ready. {link}");
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
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

    #[test]
    fn join_uses_saved_link_during_create_cooldown() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("kuro-meet-join-{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("KURO_DATA_DIR", &dir);
            std::env::set_var("KURO_MEET_OPEN", "0");
        }
        meet_last::remember(&dir.join("meet_last.json"), "https://meet.google.com/zzz-yyyy-xxx", "siya");
        crate::whatsapp::set_address_book(Some([("siya".into(), "910000000001".into())].into_iter().collect()));
        MEET_BUSY.store(false, Ordering::SeqCst);
        {
            let guard = claim_meet().expect("claim");
            guard.mark_cooldown();
        }
        assert!(claim_meet().is_none(), "create path still cooling down");
        let reply = run_join(&serde_json::json!({"contact": "siya"}));
        assert!(reply.contains("meet.google.com/zzz-yyyy-xxx") || reply.starts_with("Success:"), "got: {reply}");
        assert!(!reply.contains("already opening"));
        crate::whatsapp::set_address_book(None);
        unsafe {
            std::env::remove_var("KURO_MEET_OPEN");
            std::env::remove_var("KURO_DATA_DIR");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
