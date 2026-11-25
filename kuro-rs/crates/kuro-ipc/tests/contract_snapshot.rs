//! Snapshot HTTP/API and HUD event frames against `contract/v1/*.json`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kuro_core::EngineConfig;
use kuro_ipc::{AppState, HudSnap, event_frame, router};
use kuro_tools::brain::Brain;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tower::ServiceExt;

fn contract_dir() -> PathBuf {
    // crates/kuro-ipc -> kuro-rs -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contract/v1")
}

fn state(dir: &Path) -> Arc<AppState> {
    Arc::new(AppState {
        token: "test-token-value".into(),
        config: Mutex::new(EngineConfig {
            llm_mode: "fallback".into(),
            stt_backend: "auto".into(),
            tts_engine: "auto".into(),
            xtts_language: "en".into(),
            paused: false,
            hud_show_text: false,
            plug_id: String::new(),
        }),
        hud: Mutex::new(HudSnap::default()),
        contacts: Mutex::new(dir.join("contacts.json")),
        workflows: Mutex::new(dir.join("n8n_workflows.json")),
        timing: Mutex::new(kuro_core::Timing::new(dir.join("timing.csv"), false)),
        brain: Mutex::new(Brain::default()),
        dismiss_hits: Mutex::new(0),
        shutdown_hits: Mutex::new(0),
        shutdown_hook: Mutex::new(None),
        clock: Arc::new(Mutex::new(kuro_core::TurnClock::default())),
        turn_busy: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        hud_wake: Arc::new(Notify::new()),
    })
}

async fn call(
    app: axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header("x-kuro-token", token);
    }
    let request =
        builder.header("content-type", "application/json").body(Body::from(body.unwrap_or("").to_string())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let value = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| json!({"_raw": text}))
    };
    (status, value)
}

fn scrub(mut value: Value) -> Value {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (k, child) in map.iter_mut() {
                    if matches!(k.as_str(), "logs" | "pid" | "uptime" | "started_at") {
                        *child = json!("<volatile>");
                    } else {
                        walk(child);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item);
                }
            }
            _ => {}
        }
    }
    walk(&mut value);
    value
}

fn canonical(value: &Value) -> String {
    // serde_json preserves insertion order; re-parse via sorted keys for stability.
    fn sort(v: &Value) -> Value {
        match v {
            Value::Object(map) => {
                let mut keys: Vec<_> = map.keys().cloned().collect();
                keys.sort();
                let mut out = serde_json::Map::new();
                for k in keys {
                    out.insert(k.clone(), sort(&map[&k]));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(sort).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string_pretty(&sort(value)).unwrap() + "\n"
}

fn assert_fixture(name: &str, actual: &Value) {
    let path = contract_dir().join(name);
    let scrubbed = scrub(actual.clone());
    let rendered = canonical(&scrubbed);
    let bless = std::env::var("KURO_BLESS").ok().as_deref() == Some("1");
    if bless || !path.is_file() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &rendered).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    assert_eq!(expected, rendered, "fixture drift in {}", path.display());
}

#[tokio::test]
async fn contract_http_snapshots() {
    let dir = std::env::temp_dir().join(format!("kuro-contract-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let token = Some("test-token-value");

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/status", token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("status.ok.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "POST", "/handshake", token, Some(r#"{"protocol":1,"engine":"kuro-shell"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("handshake.ok.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "POST", "/handshake", token, Some(r#"{"protocol":99,"engine":"kuro-shell"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("accepted").and_then(|v| v.as_bool()) == Some(false));
    assert_fixture("handshake.rejected.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/config", token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("config.get.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "PUT", "/config", token, Some(r#"{"paused":false,"llm_mode":"fallback"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("config.put.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/workflows", token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("workflows.get.empty.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/contacts", token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("contacts.get.empty.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/timing", token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("timing.get.empty.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "POST", "/say", token, Some(r#"{"text":"pause the song"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("say.pause.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "POST", "/thread/clear", token, Some("{}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("thread.clear.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "POST", "/dismiss", token, Some("{}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("dismiss.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "POST", "/shutdown", token, Some("{}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_fixture("shutdown.json", &json!({"status": status.as_u16(), "body": body}));

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/status", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_fixture("status.unauthorized.json", &json!({"status": status.as_u16(), "body": body}));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contract_event_frames() {
    for state_name in ["hidden", "listening", "thinking", "speaking", "dormant", "error", "ready", "done", "idle"] {
        let hud = HudSnap {
            state: state_name.into(),
            label: state_name.into(),
            levels: vec![0.0; 11],
            you: "hi".into(),
            kuro: "hello".into(),
        };
        let with_text = event_frame(&hud, false, true);
        let without_text = event_frame(&hud, false, false);
        assert_fixture(&format!("event.{state_name}.with_text.json"), &with_text);
        assert_fixture(&format!("event.{state_name}.no_text.json"), &without_text);
    }
}
