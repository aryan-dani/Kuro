# Phase 14: Split router and TTS

- Size: M
- Depends on: phase 13
- Behavior change: none. File and module splits; optional TTS engine folder layout
  for adapters.
- ADRs: 0002
- Protected paths: none.

## Goal

Bring `phrases`/`semantic` and large TTS modules under size budgets; introduce
`engines/<id>/` layout for TTS adapters with a single registry line each.

## Scope

In: Split plans from file 01-split-plans for kuro-router and kuro-tts; corpus and
tokenizer tests stay green; TTS adapter README; size allowlist updates.

Out: New TTS engines; STT splits unless listed; protocol work (15).

## Context card

Read:
- `docs/plans/01-split-plans.md` (router, tts)
- `kuro-rs/crates/kuro-router/src/phrases.rs`, `semantic.rs`
- `kuro-rs/crates/kuro-tts/src/**`
- Corpus and kokoro tests

Create: smaller modules (`normalize.rs`, `exact.rs`, `semantic/{embed,score}.rs`,
`engines/kokoro/`, `engines/sapi/`, etc. as planned).

Edit: lib.rs re-exports; allowlist; optional tts README.

Do not touch: engine composition beyond imports; shell; features.

Commands:
```powershell
cargo test -p kuro-router
cargo test -p kuro-tts
cargo xtask check size
cargo xtask gate fast
```

## Tests first

- Router corpus + parity + route_order unchanged.
- Kokoro vocab/tokenizer tests unchanged (including `\u{2014}` handling).

## Steps

1. Split router pure functions into files; keep public API stable.
2. Split TTS into engine folders; registry selects engine from settings.
3. Shrink allowlist; document add-TTS-adapter path in AGENTS where-to-work.

## Acceptance

- No file over 400 without allowlist; targets trending under 200.
- All router/TTS tests green; no corpus edits.

## Risks

- Semantic embed file still large: split data from logic; do not regenerate
  embeddings in this phase.
