use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::{Value, json};

use crate::{AppState, authed, deny};

fn say_execute_enabled() -> bool {
    std::env::var("KURO_SAY_EXECUTE").ok().as_deref() == Some("1")
}

/// Semantic ONNX/hash hits only; pattern-only phrases stay unmatched until phase 09.
pub(crate) fn say_semantic_matched(text: &str) -> bool {
    kuro_router::route_semantic(&kuro_router::normalize(text)).is_some()
}

pub(crate) fn say_execute_blocked(text: &str) -> bool {
    let normalized = kuro_router::normalize(text);
    if let Some(kuro_router::RouteDecision::Tool(intent)) = kuro_router::route_semantic(&normalized) {
        return matches!(
            intent,
            kuro_router::Intent::SendWhatsapp
                | kuro_router::Intent::MakeMeet
                | kuro_router::Intent::JoinMeet
                | kuro_router::Intent::RunWorkflow
        );
    }
    kuro_tools::match_registered_phrase(text)
        .is_some_and(|(tool, _)| matches!(tool.name, "send_whatsapp" | "make_meet" | "join_meet" | "run_workflow"))
}

pub async fn say(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let Ok(payload) = serde_json::from_str::<Value>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let Some(text) = payload.get("text").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let mode = payload.get("mode").and_then(|v| v.as_str()).unwrap_or("dry_run");
    if !matches!(mode, "dry_run" | "execute") {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    }
    if mode == "dry_run" {
        return Json(json!({
            "matched": say_semantic_matched(text),
            "reply": "",
            "dry_run": true,
        }))
        .into_response();
    }
    if !say_execute_enabled() {
        return (StatusCode::FORBIDDEN, Json(json!({"detail": "say execute disabled"}))).into_response();
    }
    if say_execute_blocked(text) {
        return (StatusCode::FORBIDDEN, Json(json!({"detail": "say execute refused"}))).into_response();
    }
    let text_owned = text.to_string();
    let handled = tokio::task::spawn_blocking(move || {
        if let Ok(ms) = std::env::var("KURO_SAY_TEST_DELAY_MS")
            && let Ok(ms) = ms.parse::<u64>()
        {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        kuro_router::try_handle(&text_owned)
    })
    .await
    .ok()
    .flatten();
    match handled {
        Some(result) => Json(json!({"matched": true, "reply": result.reply, "dry_run": false})).into_response(),
        None => Json(json!({"matched": false, "reply": "", "dry_run": false})).into_response(),
    }
}
