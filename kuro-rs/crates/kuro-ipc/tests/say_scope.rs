use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kuro_core::EngineConfig;
use kuro_ipc::{AppState, HudSnap, router};
use kuro_tools::brain::Brain;
use tokio::sync::Notify;
use tower::ServiceExt;

static TOOL_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn test_state(dir: &Path) -> Arc<AppState> {
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

async fn post_say(app: axum::Router, body: &str) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/say")
        .header("x-kuro-token", "test-token-value")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap();
    (status, value)
}

async fn get_status(app: axum::Router) -> StatusCode {
    let request = Request::builder()
        .method("GET")
        .uri("/status")
        .header("x-kuro-token", "test-token-value")
        .body(Body::empty())
        .unwrap();
    app.oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn say_defaults_to_dry_run() {
    let dir = std::env::temp_dir().join(format!("kuro-say-dry-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    TOOL_CALLS.store(0, Ordering::SeqCst);
    kuro_tools::set_executor(Some(Box::new(|_name, _args| {
        TOOL_CALLS.fetch_add(1, Ordering::SeqCst);
        "stub".into()
    })));
    let app = router(test_state(&dir));
    let (status, body) = post_say(app, r#"{"text":"check my inbox"}"#).await;
    kuro_tools::set_executor(None);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["dry_run"], true);
    assert_eq!(body["matched"], true);
    assert_eq!(TOOL_CALLS.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn say_refuses_send_tools() {
    let dir = std::env::temp_dir().join(format!("kuro-say-refuse-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _guard = ENV_LOCK.lock().await;
    let prev = std::env::var("KURO_SAY_EXECUTE").ok();
    unsafe {
        std::env::set_var("KURO_SAY_EXECUTE", "1");
    }
    let app = router(test_state(&dir));
    let (status, _) = post_say(app, r#"{"text":"send my clipboard to sam on whatsapp","mode":"execute"}"#).await;
    unsafe {
        match prev {
            Some(v) => std::env::set_var("KURO_SAY_EXECUTE", v),
            None => std::env::remove_var("KURO_SAY_EXECUTE"),
        }
    }
    assert_eq!(status, StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn say_execute_requires_opt_in() {
    let dir = std::env::temp_dir().join(format!("kuro-say-403-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _guard = ENV_LOCK.lock().await;
    let prev = std::env::var("KURO_SAY_EXECUTE").ok();
    unsafe {
        std::env::remove_var("KURO_SAY_EXECUTE");
    }
    let app = router(test_state(&dir));
    let (status, _) = post_say(app, r#"{"text":"pause the song","mode":"execute"}"#).await;
    unsafe {
        match prev {
            Some(v) => std::env::set_var("KURO_SAY_EXECUTE", v),
            None => std::env::remove_var("KURO_SAY_EXECUTE"),
        }
    }
    assert_eq!(status, StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn say_execute_runs_allowed_phrase() {
    let dir = std::env::temp_dir().join(format!("kuro-say-run-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _guard = ENV_LOCK.lock().await;
    let prev = std::env::var("KURO_SAY_EXECUTE").ok();
    unsafe {
        std::env::set_var("KURO_SAY_EXECUTE", "1");
    }
    let app = router(test_state(&dir));
    let (status, body) = post_say(app, r#"{"text":"pause the song","mode":"execute"}"#).await;
    unsafe {
        match prev {
            Some(v) => std::env::set_var("KURO_SAY_EXECUTE", v),
            None => std::env::remove_var("KURO_SAY_EXECUTE"),
        }
    }
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["dry_run"], false);
    assert_eq!(body["matched"], true);
    assert!(!body["reply"].as_str().unwrap_or("").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn status_answers_during_slow_say() {
    let dir = std::env::temp_dir().join(format!("kuro-say-slow-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _guard = ENV_LOCK.lock().await;
    let prev_exec = std::env::var("KURO_SAY_EXECUTE").ok();
    let prev_delay = std::env::var("KURO_SAY_TEST_DELAY_MS").ok();
    unsafe {
        std::env::set_var("KURO_SAY_EXECUTE", "1");
        std::env::set_var("KURO_SAY_TEST_DELAY_MS", "400");
    }
    TOOL_CALLS.store(0, Ordering::SeqCst);
    kuro_tools::set_executor(Some(Box::new(|_name, _args| {
        TOOL_CALLS.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(400));
        "slow tool".into()
    })));
    let shared = test_state(&dir);
    let app = router(shared);
    let app_bg = app.clone();
    let slow = tokio::spawn(async move { post_say(app_bg, r#"{"text":"what's the news","mode":"execute"}"#).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let status = get_status(app).await;
    kuro_tools::set_executor(None);
    unsafe {
        match prev_delay {
            Some(v) => std::env::set_var("KURO_SAY_TEST_DELAY_MS", v),
            None => std::env::remove_var("KURO_SAY_TEST_DELAY_MS"),
        }
        match prev_exec {
            Some(v) => std::env::set_var("KURO_SAY_EXECUTE", v),
            None => std::env::remove_var("KURO_SAY_EXECUTE"),
        }
    }
    assert_eq!(status, StatusCode::OK);
    let _ = slow.await;
    let _ = std::fs::remove_dir_all(&dir);
}
