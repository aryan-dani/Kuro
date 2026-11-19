# Phase 21: Barge-in and performance

- Size: L
- Depends on: phase 20
- Behavior change: yes. User can interrupt TTS; latency budgets measured with
  xtask bench. Defaults: barge-in available when listener or PTT during speak if
  safe.
- ADRs: 0014, 0008
- Protected paths: turn / audio. Trailers as needed.

## Goal

Implement barge-in (cancel speak, optionally re-listen) and establish performance
benches for STT/TTS/route latency with baselines from phase 00 measurements.

## Scope

In: Turn transitions for barge-in; TTS cancel API; bench commands in xtask;
update baseline-measurements.md; streaming LLM to TTS spike only if low risk
(otherwise document defer).

Out: Template polish (22); Jarvis features.

## Context card

Read:
- `docs/plans/baseline-measurements.md`
- `docs/adr/0014-bench-tooling.md`
- TTS engine cancel surfaces
- Turn Speaking state

Create:
- Barge-in scenario tests
- `cargo xtask bench` skeleton with route microbench + optional TTS

Edit: turn machine; TTS adapters; docs measurements.

Do not touch: feature folders except shared audio ports.

Commands:
```powershell
cargo test -p kuro-turn
cargo xtask bench
cargo xtask gate fast
```

## Tests first

- During Speaking, cancel event stops audio and reaches Idle or Listening per
  policy.
- Bench smoke runs in CI without models (microbench only).

## Steps

1. Define barge-in policy (PTT during speak vs wake).
2. Wire TTS stop; ensure epoch cancels in-flight work.
3. Add xtask bench; record numbers in baseline file.
4. Optionally spike streaming speak; gate behind flag if incomplete.

## Acceptance

- Barge-in tests green; no stuck Speaking state.
- Bench command exists; CI runs cheap subset.
- PTT path still solid.

## Risks

- Audio glitch on stop: test with fake sink; manual checklist on hardware.
