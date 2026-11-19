//! Versioned localhost API and HUD wire types (ADR 0009).
//! Golden examples live under `contract/v1/` at the repo root.

use serde::{Deserialize, Serialize};

/// Active protocol integer on the wire (`/status`, handshake, frames).
pub const PROTOCOL_VERSION: u32 = 1;

/// Negotiated protocol version (today the full integer is the major).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolVersion(pub u32);

impl ProtocolVersion {
    pub const CURRENT: Self = Self(PROTOCOL_VERSION);

    #[must_use]
    pub fn major(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn compatible_with(self, peer: Self) -> bool {
        self.major() == peer.major()
    }
}

/// Engine `HELLO` on the named pipe or WebSocket (first outbound frame).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub protocol: u32,
    pub engine: String,
}

/// Shell answer after parsing the engine hello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub protocol: u32,
    pub shell: String,
    #[serde(default)]
    pub accepted: bool,
}

/// HUD event WebSocket frame (`type` omitted means hud per ADR 0009).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudEventFrame {
    pub state: String,
    pub label: String,
    pub levels: Vec<f64>,
    pub paused: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub you: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kuro: Option<String>,
}

/// `GET /status` JSON body (additive fields may arrive later).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusResponse {
    pub ok: bool,
    pub paused: bool,
    pub state: String,
    pub api: bool,
    pub logs: String,
    #[serde(rename = "router_mode")]
    pub router_mode: String,
    #[serde(rename = "tts_active")]
    pub tts_active: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<u32>,
}

/// Optional `card` frame payload (future HUD features).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardPayload {
    pub title: String,
    pub lines: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<CardLink>,
    #[serde(rename = "image_path", skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
}
