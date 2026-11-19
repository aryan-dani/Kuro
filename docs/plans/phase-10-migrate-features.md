# Phase 10: Migrate features into folders

- Size: L
- Depends on: phase 09
- Behavior change: none. Every phrase in the golden corpus must keep the same
  tool id and reply class. Optional adapters (Meet, WhatsApp) become feature
  folders marked off in the example profile.
- ADRs: 0003, 0004
- Protected paths: none.

## Goal

Move every tool from `kuro-tools` into `kuro-features/src/features/<id>/` with
TOML phrases/replies where ADR 0004 applies; delete or hollow out `kuro-tools`
when empty; update router registration to the new registry.

## Scope

In: One PR per cluster or one large PR with per-feature commits: presence, desk,
music, time/weather, hub/inbox, meet, whatsapp, system helpers. Routing cases in
corpus stay stable. Rituals as TOML if small enough; else leave stubs.

Out: Scaffolders (11); semantic embed changes; new Jarvis features.

## Context card

Read:
- Phase 09 Feature API
- `kuro-rs/crates/kuro-tools/src/*.rs` (each tool module)
- `kuro-rs/crates/kuro-router/src/phrases.rs` (what moves to TOML)
- `kuro-rs/fixtures/phrases/*.toml` (must stay green)

Create: one folder per feature id under `kuro-features/src/features/`.

Edit: `features/mod.rs` registration; router to call feature registry; remove
`kuro-tools` when drained; workspace members.

Do not touch: turn machine, shell, IPC protocol shapes, hotkey.

Commands:
```powershell
cargo test -p kuro-router --test corpus
cargo test -p kuro-features
cargo xtask gate fast
```

## Tests first

- Corpus must fail if any tool id or spoken reply class drifts.
- Per-feature unit tests with fakes for external IO.

## Steps

1. Migrate read_only tools first (time, about).
2. Migrate reversible (desk, music, volume).
3. Migrate optional external (WhatsApp, Meet) behind feature flags / profile off.
4. Move phrase tables into feature.toml incrementally; keep phrases.rs as
   generated or thin loader until empty.
5. Remove kuro-tools dependency edges; update arch notes.

## Acceptance

- Corpus 100% pass with zero intentional golden edits (except explicit bug fixes
  already done in 02-04).
- No tool module left in kuro-tools (crate removed or empty stub removed).
- Example profile disables Meet/WhatsApp.

## Risks

- Phrase drift: require corpus in CI before merge.
- Windows-only tools: keep `win.rs` behind cfg.
