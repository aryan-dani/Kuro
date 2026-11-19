# Phase 15: Protocol contract and handshake

- Size: M
- Depends on: phase 12 (turn ids); fixtures from phase 00
- Behavior change: additive wire fields only (version, correlation id). Old shell
  must still connect if version negotiation allows, or bump both sides together.
- ADRs: 0009, 0017
- Protected paths: contract fixtures. Trailer `ADR: 0009`.

## Goal

Extract `kuro-protocol` with versioned wire types, handshake, and golden fixtures
under `contract/v1/`. Engine and shell share the contract; generic `card` frame for
future features.

## Scope

In: `kuro-protocol` crate; version constant; handshake request/response; move DTOs
from ipc/shell duplicates; contract snapshot tests; document breaking-change rule;
optional `card` payload shape.

Out: HUD/MainWindow splits (16-17); supervisor merge (18).

## Context card

Read:
- `docs/adr/0009-protocol-contract-and-handshake.md`
- `contract/v1/*.json` from phase 00
- `kuro-rs/crates/kuro-ipc/**` message types
- `KuroShell/KuroShell.Core/Protocol/**`

Create:
- `kuro-rs/crates/kuro-protocol/`
- Updated `contract/README.md` (compat rules)

Edit: ipc to use protocol types; shell DTOs to match fixtures; workspace members.

Do not touch: feature execute logic; hotkey; turn step pure logic beyond ids.

Commands:
```powershell
cargo test -p kuro-protocol
cargo test -p kuro-ipc --test contract_snapshot
dotnet test KuroShell/KuroShell.Tests
cargo xtask gate fast
```

## Tests first

- Fixtures round-trip in Rust and C#.
- Handshake rejects incompatible major version.
- Snapshot test fails on silent field rename.

## Steps

1. Define wire types and version in kuro-protocol.
2. Align shell DTOs; add handshake on connect.
3. Add `card` optional frame (title, lines, links, image path).
4. Document: breaking change bumps vN folder and ADR.

## Acceptance

- contract/v1 fixtures pass both sides.
- Correlation id present on turn-related messages (ADR 0017).
- Gate fast green including shell tests when shell changed.

## Risks

- Dual edit drift: single fixture source of truth; generate one side if needed later.
