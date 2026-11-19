//! Unit tests for version negotiation and handshake serde.

use kuro_protocol::{HandshakeRequest, HandshakeResponse, PROTOCOL_VERSION, ProtocolVersion};

#[test]
fn compatible_with_same_major() {
    assert!(ProtocolVersion(1).compatible_with(ProtocolVersion(1)));
    assert!(ProtocolVersion(PROTOCOL_VERSION).compatible_with(ProtocolVersion::CURRENT));
}

#[test]
fn compatible_with_different_major_false() {
    assert!(!ProtocolVersion(1).compatible_with(ProtocolVersion(2)));
    assert!(!ProtocolVersion::CURRENT.compatible_with(ProtocolVersion(99)));
}

#[test]
fn handshake_serde_round_trip() {
    let req = HandshakeRequest { protocol: PROTOCOL_VERSION, engine: "kuro-shell".into() };
    let req_json = serde_json::to_string(&req).unwrap();
    let req_back: HandshakeRequest = serde_json::from_str(&req_json).unwrap();
    assert_eq!(req, req_back);

    let resp = HandshakeResponse { protocol: PROTOCOL_VERSION, shell: "kuro-shell".into(), accepted: true };
    let resp_json = serde_json::to_string(&resp).unwrap();
    let resp_back: HandshakeResponse = serde_json::from_str(&resp_json).unwrap();
    assert_eq!(resp, resp_back);

    let resp_rejected = HandshakeResponse { protocol: PROTOCOL_VERSION, shell: "kuro-shell".into(), accepted: false };
    let parsed: HandshakeResponse = serde_json::from_str(&serde_json::to_string(&resp_rejected).unwrap()).unwrap();
    assert!(!parsed.accepted);
}
