use serde_json::{Value, json};

pub const SYSTEM_PROMPT: &str = "You are Kuro, a witty, dry-humored local Windows assistant. Keep replies short.";

const WAIT: &str = "Neural engine is off. Please wait while I start it.";

#[derive(Clone, Debug, Default)]
pub struct Brain {
    pub messages: Vec<Value>,
}

impl Brain {
    pub fn thread(&mut self, user_text: &str) -> &[Value] {
        if self.messages.is_empty() {
            self.messages.push(json!({"role": "system", "content": SYSTEM_PROMPT}));
        }
        self.messages.push(json!({"role": "user", "content": user_text}));
        self.trim();
        &self.messages
    }

    pub fn remember(&mut self, reply: &str) -> String {
        self.messages.push(json!({"role": "assistant", "content": reply}));
        self.trim();
        reply.to_string()
    }

    fn trim(&mut self) {
        if self.messages.is_empty() {
            return;
        }
        let system = self.messages[0].clone();
        let mut rest = self.messages[1..].to_vec();
        if rest.len() > 40 {
            rest = rest[rest.len() - 40..].to_vec();
            if rest.first().and_then(|m| m.get("role")).and_then(|v| v.as_str()) == Some("assistant") {
                rest.remove(0);
            }
        }
        self.messages = vec![system];
        self.messages.extend(rest);
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

pub fn lazy_start_line(up: bool) -> Option<&'static str> {
    if up { None } else { Some(WAIT) }
}

pub fn speakable_line(raw: &str) -> String {
    raw.trim().to_string()
}

pub fn ollama_chat_url() -> String {
    let base = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    format!("{}/api/chat", base.trim_end_matches('/'))
}

pub fn chat(model: &str, messages: &[Value]) -> Result<String, String> {
    let body = json!({"model": model, "messages": messages, "stream": false}).to_string();
    let resp = ureq::post(&ollama_chat_url())
        .timeout(std::time::Duration::from_secs(60))
        .set("Content-Type", "application/json")
        .send_string(&body)
        .map_err(|_| "The neural engine started, but the reply failed.".to_string())?;
    let text = resp.into_string().map_err(|_| "The neural engine started, but the reply failed.".to_string())?;
    let data: Value =
        serde_json::from_str(&text).map_err(|_| "The neural engine started, but the reply failed.".to_string())?;
    Ok(data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("The neural engine started, but the reply failed.")
        .to_string())
}
