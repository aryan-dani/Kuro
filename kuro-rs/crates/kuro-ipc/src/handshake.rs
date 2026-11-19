//! POST `/handshake` and stream `HELLO` on pipe/WebSocket (ADR 0009).

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use kuro_protocol::{HandshakeRequest, HandshakeResponse, PROTOCOL_VERSION, ProtocolVersion};
use serde_json::json;

use crate::{AppState, authed, deny};

/// First outbound JSON on the HUD named pipe or WebSocket after auth.
#[must_use]
pub fn engine_hello() -> HandshakeRequest {
    HandshakeRequest { protocol: PROTOCOL_VERSION, engine: env!("CARGO_PKG_VERSION").into() }
}

#[must_use]
pub fn engine_hello_json() -> String {
    serde_json::to_string(&engine_hello()).expect("engine hello serializes")
}

#[must_use]
pub fn engine_hello_line() -> String {
    format!("{}\n", engine_hello_json())
}

pub async fn handshake(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if !authed(&state, &headers) {
        return deny().into_response();
    }
    let Ok(request) = serde_json::from_str::<HandshakeRequest>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok": false}))).into_response();
    };
    let peer = ProtocolVersion(request.protocol);
    let accepted = ProtocolVersion::CURRENT.compatible_with(peer);
    Json(HandshakeResponse { protocol: PROTOCOL_VERSION, shell: request.engine, accepted }).into_response()
}
