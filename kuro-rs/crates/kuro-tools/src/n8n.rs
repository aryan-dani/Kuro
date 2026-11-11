use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::{Value, json};

use kuro_core::get_close_matches;

const CUTOFF: f64 = 0.75;
const SYNC_TIMEOUT_SECS: u64 = 8;
const ASYNC_TIMEOUT_SECS: u64 = 3;
const REPLY_LIMIT: usize = 300;

thread_local! {
    static LOADED: RefCell<Option<Vec<Workflow>>> = const { RefCell::new(None) };
    static BASE: RefCell<Option<String>> = const { RefCell::new(None) };
    static TOKEN: RefCell<Option<String>> = const { RefCell::new(None) };
    static REGISTRY: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
}

#[derive(Clone, Debug)]
pub struct Workflow {
    pub id: String,
    pub webhook: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub mode: String,
    pub takes_input: bool,
    pub spoken_name: String,
}

pub fn set_loaded(rows: Option<Vec<Workflow>>) {
    LOADED.with(|slot| *slot.borrow_mut() = rows);
}

pub fn set_config(base: Option<String>, token: Option<String>) {
    BASE.with(|slot| *slot.borrow_mut() = base);
    TOKEN.with(|slot| *slot.borrow_mut() = token);
}

pub fn set_registry_path(path: Option<std::path::PathBuf>) {
    REGISTRY.with(|slot| *slot.borrow_mut() = path);
}

pub fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for ch in text.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            space = false;
        } else if !space && !out.is_empty() {
            out.push(' ');
            space = true;
        }
    }
    out.trim().to_string()
}

fn configured() -> bool {
    let base = BASE.with(|s| s.borrow().clone()).or_else(|| std::env::var("KURO_N8N_BASE_URL").ok());
    let token = TOKEN.with(|s| s.borrow().clone()).or_else(|| std::env::var("KURO_N8N_TOKEN").ok());
    base.map(|b| !b.trim().is_empty()).unwrap_or(false) && token.map(|t| !t.trim().is_empty()).unwrap_or(false)
}

fn base_url() -> String {
    BASE.with(|s| s.borrow().clone())
        .or_else(|| std::env::var("KURO_N8N_BASE_URL").ok())
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string()
}

fn token() -> String {
    TOKEN.with(|s| s.borrow().clone()).or_else(|| std::env::var("KURO_N8N_TOKEN").ok()).unwrap_or_default()
}

pub fn load_workflows() -> Vec<Workflow> {
    if let Some(rows) = LOADED.with(|s| s.borrow().clone()) {
        return rows;
    }
    let path = REGISTRY.with(|s| s.borrow().clone()).unwrap_or_else(|| kuro_core::KuroPaths::from_env().workflows());
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(data) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(raw) = data.get("workflows").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (key, spec) in raw {
        let Some(spec) = spec.as_object() else { continue };
        let webhook = spec.get("webhook").and_then(|v| v.as_str()).unwrap_or("").trim().trim_matches('/');
        if webhook.is_empty() {
            continue;
        }
        let mut aliases = Vec::new();
        if let Some(list) = spec.get("aliases").and_then(|v| v.as_array()) {
            for alias in list {
                let cleaned = if let Some(s) = alias.as_str() {
                    normalize(s)
                } else {
                    normalize(alias.to_string().trim_matches('"'))
                };
                if !cleaned.is_empty() {
                    aliases.push(cleaned);
                }
            }
        }
        let mode_raw = spec.get("mode").and_then(|v| v.as_str()).unwrap_or("sync").trim().to_ascii_lowercase();
        let mode = if mode_raw == "async" { "async" } else { "sync" };
        found.push(Workflow {
            id: key.clone(),
            webhook: webhook.to_string(),
            description: spec.get("description").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
            aliases,
            mode: mode.to_string(),
            takes_input: spec.get("takes_input").and_then(|v| v.as_bool()).unwrap_or(false),
            spoken_name: key.replace('_', " "),
        });
    }
    found
}

fn labels(workflows: &[Workflow]) -> Vec<(String, usize)> {
    let mut pairs = Vec::new();
    for (idx, wf) in workflows.iter().enumerate() {
        let a = normalize(&wf.id.replace('_', " "));
        let b = normalize(&wf.id);
        if !a.is_empty() {
            pairs.push((a, idx));
        }
        if !b.is_empty() {
            pairs.push((b, idx));
        }
        for alias in &wf.aliases {
            if !alias.is_empty() {
                pairs.push((alias.clone(), idx));
            }
        }
    }
    pairs
}

pub fn resolve_workflow<'a>(query: &str, workflows: &'a [Workflow]) -> Option<&'a Workflow> {
    let query_n = normalize(query);
    if query_n.is_empty() || workflows.is_empty() {
        return None;
    }
    let pairs = labels(workflows);
    for (label, idx) in &pairs {
        if query_n == *label {
            return Some(&workflows[*idx]);
        }
    }
    let mut choices: Vec<&str> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (label, _) in &pairs {
        if seen.insert(label.clone()) {
            choices.push(label.as_str());
        }
    }
    let hits = get_close_matches(&query_n, &choices, 1, CUTOFF);
    let hit = hits.first()?;
    pairs.iter().find(|(label, _)| label == *hit).map(|(_, idx)| &workflows[*idx])
}

fn strip_words(text: &str, drop: &[&str]) -> String {
    let mut toks: Vec<&str> = text.split_whitespace().collect();
    while toks.first().is_some_and(|t| drop.contains(t)) {
        toks.remove(0);
    }
    toks.join(" ")
}

const FILLERS: &[&str] = &["please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"];
const ARTICLES: &[&str] = &["the", "a", "an", "my", "this"];
const VERBS: &[&str] = &["run", "start", "trigger", "launch"];

fn alias_pairs(workflows: &[Workflow]) -> Vec<(String, usize)> {
    let mut pairs = labels(workflows);
    pairs.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
    pairs
}

fn match_body(body: &str, pairs: &[(String, usize)], workflows: &[Workflow]) -> Option<(usize, String)> {
    if body.is_empty() {
        return None;
    }
    for (alias, idx) in pairs {
        if body == alias {
            return Some((*idx, String::new()));
        }
    }
    for (alias, idx) in pairs {
        if workflows[*idx].takes_input && body.starts_with(&format!("{alias} ")) {
            return Some((*idx, body[alias.len() + 1..].trim().to_string()));
        }
    }
    None
}

pub fn match_phrase(text: &str, workflows: &[Workflow]) -> Option<(Workflow, String)> {
    if workflows.is_empty() {
        return None;
    }
    let body = strip_words(&normalize(text), FILLERS);
    let pairs = alias_pairs(workflows);
    if let Some((idx, extra)) = match_body(&body, &pairs, workflows) {
        return Some((workflows[idx].clone(), extra));
    }
    let toks: Vec<&str> = body.split_whitespace().collect();
    if toks.first().is_none_or(|t| !VERBS.contains(t)) {
        return None;
    }
    let rest = toks[1..].join(" ");
    if let Some((idx, extra)) = match_body(&rest, &pairs, workflows) {
        return Some((workflows[idx].clone(), extra));
    }
    let rest = strip_words(&rest, ARTICLES);
    match_body(&rest, &pairs, workflows).map(|(idx, extra)| (workflows[idx].clone(), extra))
}

#[derive(Debug)]
pub enum OpenError {
    Http(u16),
    Timeout,
    Unreachable,
}

pub type WebhookOpener = dyn Fn(&str, u64) -> Result<(u16, String), OpenError>;

pub fn dispatch(wf: &Workflow, text: &str, opener: Option<&WebhookOpener>) -> String {
    if !configured() {
        return "n8n is not configured.".into();
    }
    let mode = if wf.mode == "async" { "async" } else { "sync" };
    let timeout = if mode == "sync" { SYNC_TIMEOUT_SECS } else { ASYNC_TIMEOUT_SECS };
    let url = format!("{}/{}", base_url(), wf.webhook.trim_start_matches('/'));
    let result = if let Some(opener) = opener { opener(&url, timeout) } else { live_open(&url, text, timeout) };
    match result {
        Err(OpenError::Http(code)) if code == 401 || code == 403 => "n8n rejected my token.".into(),
        Err(OpenError::Http(404)) => "That workflow is not active in n8n.".into(),
        Err(OpenError::Http(code)) => format!("n8n returned HTTP {code}."),
        Err(OpenError::Timeout) => "n8n timed out.".into(),
        Err(OpenError::Unreachable) => "I can't reach n8n.".into(),
        Ok(_) if mode == "async" => {
            format!("Success: Started {}.", if wf.spoken_name.is_empty() { "workflow" } else { &wf.spoken_name })
        }
        Ok((_status, raw)) => {
            if raw.trim().is_empty() {
                return "Success: Done, sir.".into();
            }
            let data: Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(_) => return "n8n returned something I could not read.".into(),
            };
            let Some(obj) = data.as_object() else {
                return "n8n returned something I could not read.".into();
            };
            let reply = obj.get("reply").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            if reply.is_empty() {
                "Success: Done, sir.".into()
            } else if reply.len() > REPLY_LIMIT {
                format!("Success: {}...", reply.chars().take(REPLY_LIMIT - 3).collect::<String>().trim_end())
            } else {
                format!("Success: {reply}")
            }
        }
    }
}

fn live_open(url: &str, text: &str, timeout: u64) -> Result<(u16, String), OpenError> {
    let body = json!({"text": text, "source": "kuro"}).to_string();
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(timeout)).build();
    match agent
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("X-Kuro-Token", &token())
        .send_string(&body)
    {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.into_string().unwrap_or_default();
            Ok((status, text))
        }
        Err(ureq::Error::Status(code, _)) => Err(OpenError::Http(code)),
        Err(ureq::Error::Transport(t)) if t.kind() == ureq::ErrorKind::Io => Err(OpenError::Timeout),
        Err(_) => Err(OpenError::Unreachable),
    }
}

pub fn run(arguments: &Value) -> String {
    if !configured() {
        return "n8n is not configured.".into();
    }
    let workflows = load_workflows();
    if workflows.is_empty() {
        return "No workflows are configured.".into();
    }
    let query = arguments.get("workflow").and_then(|v| v.as_str()).unwrap_or("");
    let text = arguments.get("input").and_then(|v| v.as_str()).unwrap_or("");
    let Some(wf) = resolve_workflow(query, &workflows) else {
        return "I don't know that workflow.".into();
    };
    let wf = wf.clone();
    dispatch(&wf, text, None)
}

pub fn fixture_map() -> HashMap<&'static str, &'static str> {
    HashMap::new()
}
