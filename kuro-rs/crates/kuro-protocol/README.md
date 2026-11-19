# kuro-protocol

Serde wire types for the localhost API, handshake, and HUD frames (ADR 0009).
Golden JSON lives in `contract/v1/` at the repo root; tests in `tests/fixtures.rs`
deserialize those files.

`kuro-ipc` and `KuroShell.Core` still carry duplicate DTOs until a later phase-15
slice. They may depend on this crate optionally or re-export these types when wired.