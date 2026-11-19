# Phase 02: Crash containment

- Size: M (three parts, one PR each)
- Depends on: phases 00, 01
- Behavior change: yes, deliberate defect fixes only (listed below)
- ADRs: 0007, 0017
- Protected paths touched: `kuro-rs/crates/kuro-ipc/src/lib.rs` (dismiss handler).
  Trailer `ADR: 0007`.

## Goal

A panic, a busy port, or a server that never answers must not take push-to-talk
down or leave it ignored.

## Deliberate behavior changes and their regression tests

| Fix | Finding | Test |
|-----|---------|------|
| A tool panic ends the turn, not the engine | 2 | `engine::tests::panic_in_feature_keeps_ptt` |
| Busy flags release on panic and early return | 20 | `meet::tests::busy_released_on_panic` |
| Port conflict no longer aborts; bind is retried | N5 | `engine::tests::bind_conflict_keeps_ptt` |
| Network calls time out | N3 | `inbox_hung_server_times_out`, `screen_hung_model_times_out`, `meet_hung_api_times_out` |
| A press after dismiss or Escape starts a turn | N4 | `turn_gate::tests::press_after_dismiss_starts_turn` |
| STT failure returns the HUD to idle | N7 | `turn_gate::tests::error_returns_to_idle` |
| Diagnostics go to the log file | N9 | `log::tests::audio_error_is_logged` |

## Context card

Read:
- `docs/adr/0007-panic-strategy.md`
- `kuro-rs/Cargo.toml` (`[profile.release]`)
- `kuro-rs/crates/kuro-engine/src/main.rs`: the API thread (search `expect("bind")`),
  `on_up` worker closure, the escape handler, every write to `busy` and `processing`
- `kuro-rs/crates/kuro-core/src/epoch.rs` (`TurnClock`: `begin_turn`,
  `force_unblock`, `live`) - read only
- `kuro-rs/crates/kuro-ipc/src/lib.rs`: `dismiss`
- `kuro-rs/crates/kuro-tools/src/feeds.rs` (`inbox_run`, `screen_run`),
  `meet.rs` (`MEET_BUSY`, `release_meet`, the HTTP calls)

Create:
- `kuro-rs/crates/kuro-engine/src/turn_gate.rs` (under 150 lines, with tests)
- `kuro-rs/crates/kuro-engine/src/panic_hook.rs`
- `kuro-rs/crates/kuro-engine/tests/panic_containment.rs`

Edit: `kuro-rs/Cargo.toml`, `kuro-engine/src/main.rs`, `kuro-ipc/src/lib.rs`
(dismiss only), `kuro-tools/src/feeds.rs`, `kuro-tools/src/meet.rs`,
`kuro-audio/src/lib.rs` (`eprintln!` lines only).

Do not touch: `kuro-hotkey/**`, `kuro-core/src/epoch.rs`, routing code, the corpus,
`KuroShell/**`.

Commands: `cargo xtask gate fast` during work, `cargo xtask gate full` before push.

## Part A: unwind, hook, guards

Tests first: `panic_containment.rs` registers a test tool whose `execute` panics,
runs it through `ToolRegistry::execute` in a release-profile test
(`cargo test --release -p kuro-engine --test panic_containment`), and asserts the
call returns an error string and a second call works. A second test panics while a
Meet busy guard is held and asserts the next request is not "already on it".

Steps:
1. Set `panic = "unwind"` in the release profile.
2. `panic_hook.rs`: install a hook at startup that appends location and message to
   the rotating log.
3. Replace `MEET_BUSY` set and release pairs with a guard type whose `Drop`
   releases. Keep the cooldown semantics exactly (corpus must stay green).
4. Wrap the body of the `on_up` worker thread in `catch_unwind`; on panic, log and
   fall through to the cleanup that the guard in part C performs.
5. FFI: in `wa_neonize.rs` callbacks, catch panics before returning to foreign code.

## Part B: bind retry and timeouts

Tests first: bind a listener on a free port, start the API thread function on the
same port, assert it returns no panic and reports "retrying"; release the port,
assert it binds. For timeouts, a loopback listener that accepts and never answers;
each call must return within its budget plus one second.

Steps:
1. Replace `.expect("tokio")` and `.expect("bind")` with error handling: log, wait
   5 s, retry. Push-to-talk keeps running meanwhile.
2. `inbox_run` 8 s, `screen_run` 30 s, Meet OAuth refresh and create-space 8 s each
   (connect and read). Add a function parameter or a small struct for the base URL
   so tests can redirect; production values stay the same.

## Part C: turn gate, dismiss, HUD reset

Tests first, on `TurnGate` (pure, no threads needed):
`press_after_dismiss_starts_turn`, `stale_worker_does_not_clobber_new_turn`,
`error_returns_to_idle`, `panic_in_worker_releases_gate`.

Steps:
1. `TurnGate` owns `busy`, `processing`, and the HUD reset. `begin(epoch)` returns a
   guard. The guard's `Drop` clears flags and sets the HUD idle only if its epoch is
   still the live one.
2. `main.rs` uses `TurnGate` in `on_down`, `on_up`, and the escape handler instead of
   touching the flags directly. This is the only structural change to `main.rs`.
3. `/dismiss` and Escape call `gate.cancel()`, which clears `busy` immediately. The
   old worker's late results are dropped by the existing `clock.live(epoch)` checks;
   add one before speaking if it is missing.
4. STT error path: show `error` for 1.5 s through the gate, then idle.
5. Replace non-test `eprintln!` in `kuro-audio` with `append_rotating_log`.

## Acceptance criteria

- All tests in the table pass; the corpus and snapshots are unchanged.
- Manual check by the owner (hardware): start a second process on port 8765, start
  Kuro, confirm Ctrl+Alt+Space still works; free the port, confirm the shell
  connects within 10 s.
- Manual check: ask for the inbox with the network cable out; Kuro answers within
  10 s and the next press works.
- `main.rs` line count did not grow (size ratchet).

## Rollback

Revert the PR for the failing part. Part A can be reverted alone (the profile line
and guards are independent of B and C).

## Guardrails

- `catch_unwind` may appear only in `ToolRegistry::execute`, `try_handle`, the
  worker wrapper, and the `wa_neonize.rs` FFI callbacks. Do not add it elsewhere.
- Do not change routing, replies, or cooldown lengths.
- Do not edit `epoch.rs`; if you believe it must change, stop and report.
