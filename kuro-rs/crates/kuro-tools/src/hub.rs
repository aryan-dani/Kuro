use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Value, json};

pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(8);
pub const TOOLS_TIMEOUT: Duration = Duration::from_secs(5);
pub const EXEC_TIMEOUT: Duration = Duration::from_secs(30);
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);

static SEEN: std::sync::LazyLock<Mutex<HashSet<String>>> = std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Clone, Debug)]
pub struct HubConfig {
    pub base: String,
    pub token: String,
}

impl HubConfig {
    pub fn from_env() -> Option<Self> {
        let base = std::env::var("KURO_HUB_URL").unwrap_or_else(|_| "http://100.78.60.50:18200".into());
        let token = std::env::var("KURO_HUB_TOKEN").unwrap_or_default();
        if base.trim().is_empty() || token.trim().is_empty() {
            return None;
        }
        Some(Self { base: base.trim().trim_end_matches('/').to_string(), token })
    }

    pub fn configured(&self) -> bool {
        !self.base.is_empty() && !self.token.is_empty()
    }
}

pub fn command(cfg: &HubConfig, text: &str, early: bool) -> Option<Value> {
    let url = format!("{}/v1/command", cfg.base);
    let body = json!({"text": text, "source": "laptop", "early": early}).to_string();
    post_json(&url, &cfg.token, &body, COMMAND_TIMEOUT)
}

pub fn list_tools(cfg: &HubConfig) -> Vec<Value> {
    let url = format!("{}/v1/tools", cfg.base);
    match get_json(&url, &cfg.token, TOOLS_TIMEOUT) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

pub fn execute_tool(cfg: &HubConfig, name: &str, arguments: &Value) -> String {
    if !cfg.configured() {
        return "Hub is not configured.".into();
    }
    let url = format!("{}/v1/tools/{name}", cfg.base);
    let body = json!({"arguments": arguments}).to_string();
    match post_json(&url, &cfg.token, &body, EXEC_TIMEOUT) {
        Some(data) => data.get("result").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_default(),
        None => "Hub tool failed.".into(),
    }
}

pub fn health(cfg: &HubConfig) -> bool {
    let url = format!("{}/health", cfg.base);
    get_json(&url, &cfg.token, HEALTH_TIMEOUT).is_some()
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(timeout).build()
}

fn post_json(url: &str, token: &str, body: &str, timeout: Duration) -> Option<Value> {
    let resp = agent(timeout)
        .post(url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_string(body)
        .ok()?;
    let text = resp.into_string().ok()?;
    serde_json::from_str(&text).ok()
}

fn get_json(url: &str, token: &str, timeout: Duration) -> Option<Value> {
    let resp = agent(timeout).get(url).set("Authorization", &format!("Bearer {token}")).call().ok()?;
    let text = resp.into_string().ok()?;
    serde_json::from_str(&text).ok()
}

pub fn ws_url(base: &str) -> String {
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}/v1/events")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}/v1/events")
    } else {
        format!("{base}/v1/events")
    }
}

#[derive(Debug)]
pub struct EventOutcome {
    pub speak: Option<String>,
    pub notify: Option<String>,
    pub ack: Value,
    pub duplicate: bool,
}

pub fn handle_event(event: &Value) -> EventOutcome {
    let event_id = event.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !event_id.is_empty() {
        let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        if seen.contains(&event_id) {
            return EventOutcome {
                speak: None,
                notify: None,
                ack: json!({"type": "action_result", "event_id": event_id, "ok": true, "detail": "duplicate"}),
                duplicate: true,
            };
        }
        seen.insert(event_id.clone());
        if seen.len() > 500 {
            seen.clear();
            seen.insert(event_id.clone());
        }
    }
    if event.get("type").and_then(|v| v.as_str()) == Some("pong") {
        return EventOutcome { speak: None, notify: None, ack: json!({}), duplicate: false };
    }
    let etype = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let payload = event.get("payload").cloned().unwrap_or(json!({}));
    let mut ok = true;
    let mut detail = String::new();
    let mut speak = None;
    let mut notify = None;
    match etype {
        "speak" => speak = payload.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
        "notify" => notify = payload.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
        "client_action" => crate::music::apply_action(&payload),
        _ => {
            ok = false;
            detail = format!("unknown type {etype}");
        }
    }
    EventOutcome {
        speak,
        notify,
        ack: json!({"type": "action_result", "event_id": event_id, "ok": ok, "detail": detail}),
        duplicate: false,
    }
}

pub fn reset_seen() {
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
