# Phase 13: Split engine main

- Size: M
- Depends on: phase 12
- Behavior change: none. File split only; size ratchet removes allowlist entries
  as files drop under 400 lines.
- ADRs: 0002
- Protected paths: none unless hotkey wiring files move. Trailer if needed.

## Goal

Decompose `kuro-engine` `main.rs` and other oversize modules into composition-root
files under the budgets in ADR 0002 / file 01-split-plans.md. Engine remains the
only composition root.

## Scope

In: Split per [01-split-plans.md](01-split-plans.md) engine section; wire modules;
update size_allowlist.toml; finalize ARCHITECTURE.md layer diagram.

Out: Router/TTS splits (14); shell splits (16-17).

## Context card

Read:
- `docs/plans/01-split-plans.md` (engine / main sections)
- `docs/adr/0002-crate-layering-and-size-budgets.md`
- `kuro-rs/crates/kuro-engine/src/main.rs` (and lib modules)
- `kuro-rs/xtask/data/size_allowlist.toml`

Create: modules such as `supervisor.rs`, `wiring.rs`, `cli_args.rs`, `channels.rs`
as named in the split plan.

Edit: main.rs becomes short; allowlist shrinks; ARCHITECTURE.md.

Do not touch: turn step logic (already in kuro-turn); feature folders; shell.

Commands:
```powershell
cargo xtask check size
cargo test -p kuro-engine
cargo xtask gate fast
```

## Tests first

- Existing engine tests and voice snapshot remain green (refactor PR: no golden
  corpus edits).

## Steps

1. Extract pure helpers first; then wiring; leave main as orchestrate-only.
2. After each extract, run engine tests.
3. Remove allowlist rows when under cap; never raise caps.
4. Update ARCHITECTURE.md data-flow paragraph.

## Acceptance

- main.rs under 200 lines target (hard under 400).
- No behavior change; corpus and snapshots unchanged.
- Size check green with fewer allowlist entries.

## Risks

- Hidden init order dependencies: keep a single `build_app()` that preserves order.
