//! Named-pipe HUD stream (`\\.\pipe\KuroEngine`).

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{AppState, event_frame, handshake, skip_unchanged_quiet_frame};

const PIPE_NAME: &str = r"\\.\pipe\KuroEngine";

fn pipe_name() -> String {
    std::env::var("KURO_PIPE_NAME").unwrap_or_else(|_| PIPE_NAME.to_string())
}

pub async fn serve_named_pipe(state: Arc<AppState>) {
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
    if server.write_all(handshake::engine_hello_line().as_bytes()).await.is_err() {
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
