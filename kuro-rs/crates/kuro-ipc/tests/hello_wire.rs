//! Engine HELLO line on pipe/WebSocket (ADR 0009).

use std::path::PathBuf;

use kuro_ipc::handshake::{engine_hello, engine_hello_json, engine_hello_line};
use kuro_protocol::HandshakeRequest;
use serde_json::Value;

fn contract_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contract/v1")
}

#[test]
fn engine_hello_matches_contract_fixture() {
    let path = contract_dir().join("hello.frame.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let typed: HandshakeRequest = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(typed, engine_hello());
    let again: Value = serde_json::to_value(engine_hello()).unwrap();
    assert_eq!(fixture, again);
}

#[test]
fn engine_hello_line_is_json_plus_newline() {
    let line = engine_hello_line();
    assert!(line.ends_with('\n'));
    let body = line.trim_end();
    let parsed: HandshakeRequest = serde_json::from_str(body).unwrap();
    assert_eq!(parsed, engine_hello());
    assert_eq!(body, engine_hello_json());
}
