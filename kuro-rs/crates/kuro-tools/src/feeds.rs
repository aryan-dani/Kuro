use serde_json::{Value, json};

const FEEDS: &[(&str, &str, &str)] = &[
    ("The Verge", "tech", "https://www.theverge.com/rss/index.xml"),
    ("Hacker News", "tech", "https://news.ycombinator.com/rss"),
    ("BBC World", "world", "https://feeds.bbci.co.uk/news/world/rss.xml"),
    ("NPR News", "world", "https://feeds.npr.org/1001/rss.xml"),
];

pub fn run(arguments: &Value) -> String {
    let mut topic = arguments.get("topic").and_then(|v| v.as_str()).unwrap_or("general").trim().to_ascii_lowercase();
    if ["none", "null", "all", "everything", "general"].contains(&topic.as_str()) {
        topic = "general".into();
    }
    let mut lines = Vec::new();
    for (source, category, url) in FEEDS {
        if topic != "general"
            && topic != "ai"
            && topic != "business"
            && topic != "sports"
            && *category != topic
            && topic != "tech"
            && topic != "world"
        {
            continue;
        }
        if topic == "tech" && *category != "tech" {
            continue;
        }
        if topic == "world" && *category != "world" {
            continue;
        }
        if let Some(xml) = fetch(url) {
            for title in titles(&xml).into_iter().take(2) {
                lines.push(format!("- [{source}] {title}"));
                if lines.len() >= 6 {
                    break;
                }
            }
        }
        if lines.len() >= 6 {
            break;
        }
    }
    if lines.is_empty() {
        return "Unable to fetch the news right now.".into();
    }
    format!("Latest headlines:\n{}", lines.join("\n"))
}

fn fetch(url: &str) -> Option<String> {
    ureq::get(url).timeout(std::time::Duration::from_secs(6)).call().ok()?.into_string().ok()
}

fn titles(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in xml.split("<title>").skip(1) {
        let title = part.split("</title>").next().unwrap_or("").trim();
        let title = title.trim_start_matches("<![CDATA[").trim_end_matches("]]>");
        if !title.is_empty() && !title.eq_ignore_ascii_case("rss") {
            out.push(title.to_string());
        }
    }
    out
}

pub fn inbox_run(arguments: &Value) -> String {
    let mut mode = arguments.get("mode").and_then(|v| v.as_str()).unwrap_or("summary").trim().to_ascii_lowercase();
    if !["summary", "deadlines", "urgent"].contains(&mode.as_str()) {
        mode = "summary".into();
    }
    let url = std::env::var("N8N_INBOX_WEBHOOK_URL").unwrap_or_default();
    if url.trim().is_empty() {
        return "I couldn't reach the cloud inbox.".into();
    }
    let secret = std::env::var("N8N_WEBHOOK_SECRET").unwrap_or_default();
    let body = json!({"mode": mode}).to_string();
    let mut req = ureq::post(url.trim());
    req = req.set("Content-Type", "application/json");
    if !secret.is_empty() {
        req = req.set("X-Webhook-Secret", &secret);
    }
    let Ok(resp) = req.send_string(&body) else {
        return "I couldn't reach the cloud inbox.".into();
    };
    let Ok(text) = resp.into_string() else {
        return "I couldn't reach the cloud inbox.".into();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return text;
    };
    if data.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        return "I couldn't reach the cloud inbox.".into();
    }
    data.get("spoken")
        .or_else(|| data.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("I couldn't reach the cloud inbox.")
        .trim()
        .to_string()
}

pub fn screen_run(arguments: &Value) -> String {
    let question = arguments.get("question").and_then(|v| v.as_str()).unwrap_or("").trim();
    let Some(png) = capture_png() else {
        return "I couldn't read the screen.".into();
    };
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png);
    let model = std::env::var("OLLAMA_VISION_MODEL").unwrap_or_else(|_| "qwen3-vl:8b".into());
    let body = json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": question,
            "images": [b64],
        }],
        "stream": false
    })
    .to_string();
    let url = crate::brain::ollama_chat_url();
    let Ok(resp) = ureq::post(&url).set("Content-Type", "application/json").send_string(&body) else {
        return "I couldn't read the screen.".into();
    };
    let Ok(text) = resp.into_string() else {
        return "I couldn't read the screen.".into();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return "I couldn't read the screen.".into();
    };
    data.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("I couldn't read the screen.")
        .trim()
        .to_string()
}

fn capture_png() -> Option<Vec<u8>> {
    crate::screen::capture_primary_png()
}
