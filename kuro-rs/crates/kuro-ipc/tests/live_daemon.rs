use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::StatusCode;
use kuro_core::EngineConfig;
use kuro_ipc::{AppState, HudSnap, event_frame, serve};
use kuro_tools::brain::Brain;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::sync::Notify;
use tokio::time::timeout;

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kuro-live-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn state(dir: &std::path::Path) -> Arc<AppState> {
    Arc::new(AppState {
        token: "gate-test-token".into(),
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
        hud_wake: Arc::new(Notify::new()),
    })
}

#[tokio::test]
async fn live_daemon_http_pipe_and_auth() {
    let dir = temp_dir();
    let pipe = format!(r"\\.\pipe\KuroGateTest-{}", std::process::id());
    unsafe {
        std::env::set_var("KURO_PIPE_NAME", &pipe);
    }
    let shared = state(&dir);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_state = shared.clone();
    tokio::spawn(async move {
        serve(serve_state, listener).await;
    });
    tokio::time::sleep(Duration::from_millis(120)).await;

    let base = format!("http://127.0.0.1:{}", addr.port());
    let client = reqwest::Client::new();
    let denied = client.get(format!("{base}/status")).send().await.unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let ok = client.get(format!("{base}/status")).header("x-kuro-token", "gate-test-token").send().await.unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let status_json: serde_json::Value = serde_json::from_str(&ok.text().await.unwrap()).unwrap();
    assert!(matches!(status_json["router_mode"].as_str(), Some("semantic_onnx" | "legacy")));
    assert!(matches!(status_json["tts_active"].as_str(), Some("kokoro" | "piper" | "sapi")));

    let cfg_res = client.get(format!("{base}/config")).header("x-kuro-token", "gate-test-token").send().await.unwrap();
    assert_eq!(cfg_res.status(), StatusCode::OK);
    let cfg_json: serde_json::Value = serde_json::from_str(&cfg_res.text().await.unwrap()).unwrap();
    assert!(matches!(cfg_json["router_mode"].as_str(), Some("semantic_onnx" | "legacy")));
    assert!(matches!(cfg_json["tts_active"].as_str(), Some("kokoro" | "piper" | "sapi")));

    let say = client
        .post(format!("{base}/say"))
        .header("x-kuro-token", "gate-test-token")
        .header("content-type", "application/json")
        .body(r#"{"text":"who are you"}"#.to_string())
        .send()
        .await
        .unwrap();
    let body = say.text().await.unwrap();
    assert!(body.contains("Kuro. Local assistant"));

    {
        let mut hud = shared.hud.lock().unwrap();
        hud.state = "listening".into();
        hud.label = "listening".into();
        hud.levels = (0..11).map(|i| (i as f64) * 0.05).collect();
        hud.you = "secret".into();
        hud.kuro = "hidden".into();
    }

    let client_pipe = ClientOptions::new().open(&pipe).expect("pipe");
    let (reader, mut writer) = tokio::io::split(client_pipe);
    writer.write_all(b"AUTH gate-test-token\n").await.unwrap();
    let mut lines = BufReader::new(reader).lines();
    let auth = timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("auth timeout")
        .expect("auth line")
        .expect("auth payload");
    assert_eq!(auth, "OK");
    let frame_line = timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("frame timeout")
        .expect("frame line")
        .expect("frame payload");
    let frame: serde_json::Value = serde_json::from_str(&frame_line).unwrap();
    assert_eq!(frame["state"], "listening");
    assert_eq!(frame["levels"].as_array().unwrap().len(), 11);
    assert!(frame.get("you").is_none());

    let hud = shared.hud.lock().unwrap().clone();
    let quiet = event_frame(&hud, false, false);
    assert!(!quiet.to_string().contains("secret"));
    let _ = json!({});
    let _ = std::fs::remove_dir_all(&dir);
}
