use std::path::Path;

use serde_json::Value;

/// HTTP endpoints for Meet API calls (overridable in tests).
#[derive(Clone, Debug)]
pub struct MeetHttp {
    pub token_url: String,
    pub spaces_url: String,
}

impl MeetHttp {
    pub fn production() -> Self {
        Self {
            token_url: "https://oauth2.googleapis.com/token".into(),
            spaces_url: "https://meet.googleapis.com/v2/spaces".into(),
        }
    }
}

pub(crate) fn access_token(path: &Path, http: &MeetHttp) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    if !refresh_if_needed(path, &data, http) {
        // Still try whatever token is on disk.
    }
    let text = std::fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    let token = data.get("token").or_else(|| data.get("access_token")).and_then(|v| v.as_str()).unwrap_or("").trim();
    if token.is_empty() { None } else { Some(token.to_string()) }
}

pub(crate) fn google_token_usable(path: &Path) -> bool {
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
        return refresh_token(path, &data, refresh, &MeetHttp::production());
    }
    !token.is_empty() || !refresh.is_empty()
}

fn refresh_if_needed(path: &Path, data: &Value, http: &MeetHttp) -> bool {
    let refresh = data.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("");
    if refresh.is_empty() || !token_expired(data) {
        return true;
    }
    refresh_token(path, data, refresh, http)
}

pub(crate) fn token_expired(data: &Value) -> bool {
    let Some(expiry) = data.get("expiry").and_then(|v| v.as_str()) else {
        return true;
    };
    let Some(expires_at) = parse_expiry(expiry) else {
        return true;
    };
    let now = chrono::Utc::now();
    now > expires_at + chrono::Duration::seconds(60)
}

fn parse_expiry(expiry: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if expiry.contains('T') {
        return chrono::DateTime::parse_from_rfc3339(expiry).ok().map(|dt| dt.with_timezone(&chrono::Utc));
    }
    if expiry.len() >= 10 {
        return expiry.parse::<i64>().ok().and_then(|secs| chrono::DateTime::from_timestamp(secs, 0));
    }
    None
}

fn refresh_token(path: &Path, data: &Value, refresh: &str, http: &MeetHttp) -> bool {
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
    let resp = ureq::post(&http.token_url)
        .timeout(std::time::Duration::from_secs(8))
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

pub fn create_space(access_token: &str, http: &MeetHttp) -> Option<String> {
    let resp = ureq::post(&http.spaces_url)
        .timeout(std::time::Duration::from_secs(8))
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Content-Type", "application/json")
        .send_string("{}")
        .ok()?;
    let text = resp.into_string().ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    let link = data.get("meetingUri").and_then(|v| v.as_str()).unwrap_or("");
    if link.starts_with("https://") { Some(link.to_string()) } else { None }
}
