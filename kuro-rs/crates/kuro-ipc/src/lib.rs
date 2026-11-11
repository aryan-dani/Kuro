use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use kuro_core::{EngineConfig, Timing, apply_config, mask_number};
use kuro_tools::brain::Brain;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct HudSnap {
    pub state: String,
    pub label: String,
    pub levels: Vec<f64>,
    pub you: String,
    pub kuro: String,
}

impl Default for HudSnap {
    fn default() -> Self {
        Self {
            state: "hidden".into(),
            label: "hidden".into(),
            levels: vec![0.0; 11],
            you: String::new(),
            kuro: String::new(),
        }
    }
}

pub struct AppState {
    pub token: String,
    pub config: Mutex<EngineConfig>,
    pub hud: Mutex<HudSnap>,
    pub contacts: Mutex<PathBuf>,
    pub workflows: Mutex<PathBuf>,
    pub timing: Mutex<Timing>,
    pub brain: Mutex<Brain>,
    pub dismiss_hits: Mutex<u32>,
    pub shutdown_hits: Mutex<u32>,
    pub shutdown_hook: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
    /// Shared with the hotkey loop so Escape / `/dismiss` can cancel a live turn.
    pub clock: std::sync::Arc<Mutex<kuro_core::TurnClock>>,
}

pub fn event_frame(hud: &HudSnap, paused: bool, include_text: bool) -> Value {
    let mut levels = hud.levels.clone();
    if levels.len() < 11 {
        levels.resize(11, 0.0);
    }
    levels.truncate(11);
    let mut frame = json!({
        "state": hud.state,
        "label": if hud.label.is_empty() { hud.state.clone() } else { hud.label.clone() },
        "levels": levels,
        "paused": paused,
    });
    if include_text {
        frame["you"] = json!(hud.you);
        frame["kuro"] = json!(hud.kuro);
    }
    frame
}

fn authed(state: &AppState, headers: &HeaderMap) -> bool {
    let presented = headers.get("x-kuro-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    kuro_core::token_matches(&state.token, presented)
}

fn deny() -> (StatusCode, Json<Value>) {
    (StatusCode::UNAUTHORIZED, Json(json!({"detail": "unauthorized"})))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/config", get(get_config).put(put_config))
        .route("/workflows", get(get_workflows).put(put_workflows))
        .route("/contacts", get(get_contacts).put(put_contacts))
        .route("/timing", get(get_timing))
        .route("/say", post(say))
        .route("/thread/clear", post(clear_thread))
        .route("/dismiss", post(dismiss))
        .route("/shutdown", post(shutdown))
        .route("/events", get(events))
        .with_state(state)
}

async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let hud = state.hud.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let paused = state.config.lock().unwrap_or_else(|e| e.into_inner()).paused;
    Json(json!({
        "ok": true,
        "paused": paused,
        "state": hud.state,
        "api": true,
        "logs": kuro_core::local_root().display().to_string(),
    }))
    .into_response()
}

async fn get_config(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let cfg = state.config.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Json(kuro_core::public_config(&cfg)).into_response()
}

async fn put_config(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let Ok(payload) = serde_json::from_str::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    if !payload.is_object() {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    let mut cfg = state.config.lock().unwrap_or_else(|e| e.into_inner());
    if apply_config(&mut cfg, &payload).is_err() {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    Json(kuro_core::public_config(&cfg)).into_response()
}

fn read_json(path: &std::path::Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn atomic_write(path: &std::path::Path, text: &str) -> bool {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_err() {
        return false;
    }
    std::fs::rename(&tmp, path).is_ok()
}

async fn get_workflows(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let path = state.workflows.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if !path.is_file() {
        return Json(json!({"workflows": {}})).into_response();
    }
    match read_json(&path) {
        Some(data) if data.is_object() => Json(data).into_response(),
        Some(_) => Json(json!({"workflows": {}})).into_response(),
        None => (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response(),
    }
}

fn workflows_ok(data: &Value) -> bool {
    let Some(raw) = data.get("workflows").and_then(|v| v.as_object()) else {
        return false;
    };
    for (key, spec) in raw {
        if key.is_empty() || !spec.is_object() {
            return false;
        }
        let webhook = spec.get("webhook").and_then(|v| v.as_str()).unwrap_or("");
        if webhook.contains("://") {
            return false;
        }
    }
    true
}

async fn put_workflows(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let Ok(payload) = serde_json::from_str::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    if !workflows_ok(&payload) {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    let path = state.workflows.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into()) + "\n";
    if !atomic_write(&path, &text) {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    Json(json!({"ok": true})).into_response()
}

fn contacts_ok(data: &serde_json::Map<String, Value>) -> bool {
    if data.is_empty() {
        return false;
    }
    for (key, value) in data {
        if key.trim().is_empty() || !value.is_string() {
            return false;
        }
        let digits: String = value.as_str().unwrap_or("").chars().filter(|c| c.is_ascii_digit()).collect();
        if !(8..=15).contains(&digits.len()) {
            return false;
        }
    }
    true
}

async fn get_contacts(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let path = state.contacts.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if !path.is_file() {
        return Json(json!({"contacts": {}})).into_response();
    }
    let Some(data) = read_json(&path) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let Some(obj) = data.as_object() else {
        return Json(json!({"contacts": {}})).into_response();
    };
    let mut masked = serde_json::Map::new();
    for (key, value) in obj {
        masked.insert(key.clone(), json!(mask_number(value.as_str().unwrap_or(&value.to_string()))));
    }
    Json(json!({"contacts": masked})).into_response()
}

async fn put_contacts(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let Ok(payload) = serde_json::from_str::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let Some(incoming) = payload.as_object() else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let path = state.contacts.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut existing = read_json(&path).and_then(|v| v.as_object().cloned()).unwrap_or_default();
    for (key, value) in incoming {
        if value.as_str() == Some("") {
            existing.remove(key);
        } else {
            existing.insert(key.clone(), value.clone());
        }
    }
    if !contacts_ok(&existing) {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    let text = serde_json::to_string_pretty(&existing).unwrap_or_else(|_| "{}".into()) + "\n";
    if !atomic_write(&path, &text) {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    Json(json!({"ok": true})).into_response()
}

async fn get_timing(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let path = state.timing.lock().unwrap_or_else(|e| e.into_inner()).path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let mut parts = line.split(',');
        let _id = parts.next();
        let kind = parts.next().unwrap_or("");
        let stage = parts.next().unwrap_or("");
        let ms = parts.next().unwrap_or("");
        rows.push(json!({"kind": kind, "stage": stage, "ms": ms}));
    }
    let start = rows.len().saturating_sub(20);
    Json(json!({"rows": rows[start..].to_vec()})).into_response()
}

async fn say(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let Ok(payload) = serde_json::from_str::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let Some(text) = payload.get("text").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    match kuro_router::try_handle(text) {
        Some(result) => Json(json!({"matched": true, "reply": result.reply})).into_response(),
        None => Json(json!({"matched": false, "reply": ""})).into_response(),
    }
}

async fn clear_thread(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    state.brain.lock().unwrap_or_else(|e| e.into_inner()).clear();
    Json(json!({"ok": true})).into_response()
}

async fn dismiss(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    *state.dismiss_hits.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    if let Ok(mut clock) = state.clock.lock() {
        clock.force_unblock();
    }
    if let Ok(mut hud) = state.hud.lock() {
        hud.state = "idle".into();
        hud.label = "idle".into();
        hud.levels = vec![0.0; 11];
        hud.you.clear();
        hud.kuro.clear();
    }
    Json(json!({"ok": true})).into_response()
}

async fn shutdown(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    *state.shutdown_hits.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    if let Some(hook) = state.shutdown_hook.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        hook();
    }
    Json(json!({"ok": true})).into_response()
}

/// Skip unchanged quiet frames when the HUD is hidden/idle and all levels are zero.
pub fn skip_unchanged_quiet_frame(hud: &HudSnap, previous: Option<&str>, frame: &str) -> bool {
    let quiet =
        (hud.state == "hidden" || hud.state == "idle") && hud.levels.iter().all(|level| (*level).abs() < f64::EPSILON);
    quiet && previous == Some(frame)
}

async fn events(State(state): State<Arc<AppState>>, headers: HeaderMap, ws: WebSocketUpgrade) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    ws.on_upgrade(move |mut socket| async move {
        let mut previous: Option<String> = None;
        loop {
            let hud = state.hud.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let cfg = state.config.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let frame = event_frame(&hud, cfg.paused, cfg.hud_show_text).to_string();
            if skip_unchanged_quiet_frame(&hud, previous.as_deref(), &frame) {
                tokio::time::sleep(std::time::Duration::from_millis(33)).await;
                continue;
            }
            if socket.send(axum::extract::ws::Message::Text(frame.clone())).await.is_err() {
                break;
            }
            previous = Some(frame);
            tokio::time::sleep(std::time::Duration::from_millis(33)).await;
        }
    })
    .into_response()
}

pub fn can_listen(token: &str) -> bool {
    !token.is_empty()
}

const PIPE_NAME: &str = r"\\.\pipe\KuroEngine";

fn pipe_name() -> String {
    std::env::var("KURO_PIPE_NAME").unwrap_or_else(|_| PIPE_NAME.to_string())
}

async fn serve_named_pipe(state: Arc<AppState>) {
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe = pipe_name();
    let mut server = match ServerOptions::new().first_pipe_instance(true).create(&pipe) {
        Ok(server) => server,
        Err(_) => return,
    };
    loop {
        if server.connect().await.is_err() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            continue;
        }
        let connected = server;
        server = match ServerOptions::new().create(&pipe) {
            Ok(next) => next,
            Err(_) => return,
        };
        let client_state = state.clone();
        tokio::spawn(async move {
            handle_pipe_client(client_state, connected).await;
        });
    }
}

async fn handle_pipe_client(state: Arc<AppState>, mut server: tokio::net::windows::named_pipe::NamedPipeServer) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut auth_buf = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    loop {
        if server.read_exact(&mut byte).await.is_err() {
            return;
        }
        if byte[0] == b'\n' {
            break;
        }
        auth_buf.push(byte[0]);
        if auth_buf.len() > 512 {
            return;
        }
    }
    let auth = String::from_utf8_lossy(&auth_buf);
    let presented = auth.trim().strip_prefix("AUTH ").unwrap_or("").trim();
    if !kuro_core::token_matches(&state.token, presented) {
        let _ = server.write_all(b"DENY\n").await;
        return;
    }
    if server.write_all(b"OK\n").await.is_err() {
        return;
    }
    let mut previous: Option<String> = None;
    loop {
        let hud = state.hud.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let cfg = state.config.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let frame = event_frame(&hud, cfg.paused, cfg.hud_show_text).to_string();
        if skip_unchanged_quiet_frame(&hud, previous.as_deref(), &frame) {
            tokio::time::sleep(std::time::Duration::from_millis(33)).await;
            continue;
        }
        let mut line = frame.clone();
        line.push('\n');
        if server.write_all(line.as_bytes()).await.is_err() {
            break;
        }
        previous = Some(frame);
        tokio::time::sleep(std::time::Duration::from_millis(33)).await;
    }
}

pub async fn serve(state: Arc<AppState>, listener: tokio::net::TcpListener) {
    let pipe_state = state.clone();
    tokio::spawn(async move {
        serve_named_pipe(pipe_state).await;
    });
    let _ = axum::serve(listener, router(state)).await;
}
