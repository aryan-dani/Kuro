use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kuro_core::EngineConfig;
use kuro_ipc::{AppState, HudSnap, event_frame, router};
use kuro_tools::brain::Brain;
use serde_json::json;
use tower::ServiceExt;

fn state(dir: &std::path::Path) -> Arc<AppState> {
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
    })
}

async fn call(
    app: axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header("x-kuro-token", token);
    }
    let request =
        builder.header("content-type", "application/json").body(Body::from(body.unwrap_or("").to_string())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn atomic_contacts_and_workflows() {
    let dir = std::env::temp_dir().join(format!("kuro-api-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let app = router(state(&dir));
    let (status, _) = call(app, "PUT", "/contacts", Some("test-token-value"), Some(r#"{"me":"12"}"#)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!dir.join("contacts.json").exists());

    let app = router(state(&dir));
    let (status, _) = call(app, "PUT", "/contacts", Some("test-token-value"), Some(r#"{"me":"910000000001"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("contacts.json")).unwrap()).unwrap();
    assert_eq!(written["me"], "910000000001");

    let app = router(state(&dir));
    let (status, body) = call(app, "GET", "/contacts", Some("test-token-value"), None).await;
    assert_eq!(status, StatusCode::OK);
    let listed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(listed["contacts"]["me"].as_str().unwrap().ends_with("01"));
    assert!(!body.contains("910000000001"));

    let app = router(state(&dir));
    let (status, _) = call(app, "GET", "/contacts", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let app = router(state(&dir));
    let wf = r#"{"workflows":{"hello":{"webhook":"hello-kuro","mode":"sync"}}}"#;
    let (status, _) = call(app, "PUT", "/workflows", Some("test-token-value"), Some(wf)).await;
    assert_eq!(status, StatusCode::OK);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("n8n_workflows.json")).unwrap()).unwrap();
    assert_eq!(saved["workflows"]["hello"]["webhook"], "hello-kuro");

    let app = router(state(&dir));
    let leaked = r#"{"workflows":{"bad":{"webhook":"http://example.test/hook"}}}"#;
    let (status, _) = call(app, "PUT", "/workflows", Some("test-token-value"), Some(leaked)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!std::fs::read_to_string(dir.join("n8n_workflows.json")).unwrap().contains("example.test"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn say_uses_the_phrase_router() {
    let dir = std::env::temp_dir().join(format!("kuro-say-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let app = router(state(&dir));
    let (status, body) =
        call(app, "POST", "/say", Some("test-token-value"), Some(r#"{"text":"pause the song"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    let data: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(data["matched"], true);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn thread_clear_and_dismiss() {
    let dir = std::env::temp_dir().join(format!("kuro-clear-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let shared = state(&dir);
    shared.brain.lock().unwrap().thread("keep");
    let app = router(shared.clone());
    let (status, _) = call(app, "POST", "/thread/clear", None, Some("{}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!shared.brain.lock().unwrap().messages.is_empty());
    let app = router(shared.clone());
    let (status, _) = call(app, "POST", "/thread/clear", Some("test-token-value"), Some("{}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(shared.brain.lock().unwrap().messages.is_empty());

    let app = router(shared.clone());
    let (status, _) = call(app, "POST", "/dismiss", None, Some("{}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(*shared.dismiss_hits.lock().unwrap(), 0);
    let app = router(shared.clone());
    let (status, _) = call(app, "POST", "/dismiss", Some("test-token-value"), Some("{}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(*shared.dismiss_hits.lock().unwrap(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn event_frame_hides_text_unless_asked() {
    let hud = HudSnap {
        state: "listening".into(),
        label: "listening".into(),
        levels: vec![0.1; 11],
        you: "secret phrase".into(),
        kuro: "reply phrase".into(),
    };
    let quiet = event_frame(&hud, false, false);
    assert!(!quiet.to_string().contains("secret"));
    let shown = event_frame(&hud, false, true);
    assert_eq!(shown["you"], "secret phrase");
    let _ = json!({});
}
