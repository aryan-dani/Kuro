//! Bind wire types to `contract/v1/*.json` (ADR 0009).

use std::path::PathBuf;

use kuro_protocol::{HandshakeRequest, HudEventFrame};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn contract_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contract/v1")
}

fn round_trip_fixture<T>(name: &str)
where
    T: Serialize + DeserializeOwned + std::fmt::Debug,
{
    let path = contract_dir().join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    let typed: T = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("into {}: {e}", path.display()));
    let again: Value = serde_json::to_value(&typed).unwrap_or_else(|e| panic!("re-serialize {}: {e}", path.display()));
    assert_eq!(value, again, "round-trip drift for {}", path.display());
}

#[test]
fn engine_hello_matches_contract() {
    round_trip_fixture::<HandshakeRequest>("hello.frame.json");
}

#[test]
fn hud_ready_with_text_matches_contract() {
    round_trip_fixture::<HudEventFrame>("event.ready.with_text.json");
}

#[test]
fn hud_ready_no_text_matches_contract() {
    round_trip_fixture::<HudEventFrame>("event.ready.no_text.json");
}

#[test]
fn hud_listening_with_text_matches_contract() {
    round_trip_fixture::<HudEventFrame>("event.listening.with_text.json");
}
