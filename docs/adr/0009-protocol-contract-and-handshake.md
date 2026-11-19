# ADR 0009: Protocol contract and handshake

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 5; finding S4; phases 00, 15

## Context

The engine builds JSON by hand (`event_frame`, route handlers in
`kuro-ipc/src/lib.rs`). The shell mirrors the shapes in `EngineJson.cs`. Nothing
checks that they agree, there is no version, and frames have no type. The owner
leans toward a versioned contract (OpenAPI or JSON Schema) with contract tests on
both sides and a handshake.

## Options

1. OpenAPI with generated server stubs and a generated C# client. Strong, but adds
   two generators and their upgrade churn for 12 routes and one frame.
2. JSON Schema as the source, validators on both sides. Still needs a schema library
   in two languages.
3. Golden JSON fixtures as the contract, typed structs on both sides, tests that
   bind each side to the same files.

## Decision

Option 3, with a generated JSON Schema for documentation only.

- `kuro-protocol` owns every wire type as serde structs.
- `contract/v1/*.json` are canonical examples of every request, response, and frame.
- Rust: serialize canonical values and compare with the files; deserialize the files.
- C#: deserialize the same files into the DTOs in `KuroShell.Core/Protocol` with
  unknown members disallowed, and serialize requests back to the same bytes.
- Version: `protocol` integer in `/status`; first message on the pipe and WebSocket
  is `HELLO {"protocol":1,"engine":"x.y.z"}`. The shell refuses a different major
  version and says so. Additive changes do not bump the version.
- Frames carry `type` (`hud`, `turn`, `health`, `config`, `announcement`, `card`). A
  missing `type` means `hud`, so the current shape is valid version 1.
- Unknown frame types are ignored by the shell, never fatal.

Revisit generation if the DTO count passes about 25 or a second client appears.

## Consequences

- A wire change is a visible diff under `contract/`, which is PROTECTED.
- No new tooling. The cost is writing fixtures by hand, once.
- The DTOs move from the WinUI project to `KuroShell.Core` so tests can load them.

## Enforced by

`kuro-protocol/tests/fixtures.rs`; `ContractTests` in `KuroShell.Tests`; PROTECTED
status of `contract/` and `kuro-protocol/`.
