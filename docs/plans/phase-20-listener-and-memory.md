# Phase 20: Listener mode and conversation memory

- Size: L
- Depends on: phase 19
- Behavior change: yes, product. Continuous listen mode (opt-in) and recall
  features (repeat / again / undo / last question). Defaults keep today's PTT-only
  behavior.
- ADRs: 0008 (extensions), 0016 (undo tier), open Q on memory
- Protected paths: turn machine. Trailer `ADR: 0008`.

## Goal

Add opt-in listener loop on the turn machine; persist a turn log; ship recall
feature folder. Outcome may carry `undo` closures for reversible actions.

## Scope

In: Turn states/extensions for ListenIdle; Store-backed turn log; `recall`
feature; config keys `listener.*`, `recall.*`; tests with FakeMic / FakeStore.

Out: Barge-in and streaming TTS (21); timers (23); full confirm flow (26A).

## Context card

Read:
- `docs/plans/05-jarvis-roadmap.md` items 2 and model gaps
- `kuro-turn` states from phase 12
- Feature scaffolder

Create:
- `features/recall/` with feature.toml and tests
- Turn log store adapter (SQLite)
- Scenario tests for repeat/again/undo

Edit: turn machine; settings; example profile defaults off for listener.

Do not touch: wake word (25); calendar (24).

Commands:
```powershell
cargo test -p kuro-turn
cargo test -p kuro-features
cargo xtask gate fast
```

## Tests first

- PTT-only default: existing snapshots unchanged.
- Recall.repeat speaks last utterance; undo of desk power with fake device.
- Listener cancel returns to idle; never stacks turns.

## Steps

1. Turn log schema and Store port implementation.
2. Recall feature using log.
3. Listener mode behind flag; reuse Trigger port.
4. Document voice phrases in feature.toml.

## Acceptance

- Defaults preserve PTT behavior (corpus + snapshots).
- Recall works with tests; undo refuses non-undoable with spoken reason.
- Listener opt-in only.

## Risks

- Privacy: turn log local only; document retention key `recall.keep_turns`.
