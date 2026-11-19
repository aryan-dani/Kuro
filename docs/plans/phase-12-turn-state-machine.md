# Phase 12: Turn state machine and ports

- Size: L
- Depends on: phase 11
- Behavior change: none intended. Voice turn orchestration moves behind a pure
  state machine and ports; ordering must match phase 00 voice_turn_snapshot.
- ADRs: 0008, 0007, 0017
- Protected paths: turn/epoch once marked protected. Trailer `ADR: 0008`.

## Goal

Extract `kuro-turn` with a pure `step` function, explicit states, Trigger port,
FollowUp policy, correlation ids on turns. Engine becomes a thin runner over ports.

## Scope

In: `kuro-turn` crate; states Idle, Listening, Transcribing, Routing, Executing,
Speaking, FollowUp, Cancelling (exact set per ADR 0008); ports for Mic, Stt, Tts,
Router, Features, Clock, Announcer stub; scenario tests; wire engine VoiceTurn to
the machine; preserve epoch cancel.

Out: Split of main.rs file layout (13); protocol version bump (15) unless needed;
listener mode product work (20).

## Context card

Read:
- `docs/adr/0008-turn-state-machine-and-ports.md`
- `docs/adr/0007-panic-strategy.md`
- `kuro-rs/crates/kuro-engine/src/voice.rs` (and related)
- `kuro-rs/crates/kuro-engine/tests/voice_turn_snapshot.rs`
- `kuro-rs/crates/kuro-engine/tests/loop_stages.rs`
- `kuro-rs/crates/kuro-core/src/epoch.rs`

Create:
- `kuro-rs/crates/kuro-turn/` (state, step, ports, follow_up, tests/scenarios)
- `.cursor/rules/turn.mdc`

Edit: `kuro-engine` to call turn runner; workspace members.

Do not touch: hotkey hook internals; shell; feature folder contents.

Commands:
```powershell
cargo test -p kuro-turn
cargo test -p kuro-engine --test voice_turn_snapshot
cargo xtask gate fast
```

## Tests first

- Scenario matrix: happy path, cancel from each state, STT fail, tool fail, TTS
  fail, follow-up expect slot.
- Snapshot test unchanged vs phase 00 ordering.
- step is pure: no I/O in unit tests without fakes.

## Steps

1. Define State and Event enums; implement step.
2. Define port traits in kuro-core or kuro-turn.
3. Adapter implementations wrapping existing STT/TTS/router/features.
4. Replace VoiceTurn::run body with runner loop + catch_unwind boundary (ADR 0007).
5. Add correlation id field (ADR 0017) to turn context logs.

## Acceptance

- voice_turn_snapshot green without golden edits.
- No thread::spawn inside features still holds.
- Cancel reaches Idle from every state in tests.

## Risks

- Hidden side effects in current voice.rs: characterize with snapshot before edit.
- Over-wide State enum: keep minimal; extend in phase 20/26.
