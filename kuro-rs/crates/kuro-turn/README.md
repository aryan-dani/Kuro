# kuro-turn

Pure voice turn state machine (ADR 0008). `step(state, event)` returns the next
state and port effects; it performs no I/O.

## Engine wiring (later slice)

1. Hotkey thread calls `step(Idle, HotkeyDown)` and runs `StartMic` through the Mic
   port. HUD updates go through `SetHud`.
2. On release, `HotkeyUp` leads to `Transcribe`; the STT worker posts `SttDone` or
   `Error` with the turn epoch attached.
3. `Route` and `Execute` map to the router and feature runner ports; completion is
   `RouteDone`.
4. `Speak` runs on the TTS port; `SpeakDone` ends the turn unless follow-up policy
   applies.
5. Dismiss, Escape, and API cancel inject `Cancel` once; stale worker events are
   dropped when `TurnClock::live(epoch)` is false.

`TurnRunner` in `runner.rs` is a stub until `VoiceTurn::run` delegates here.

## Listener mode (phase 20)

The machine has a `FollowUp` state for short listen-after-speak windows when a
route sets `RouteOutcome.allow_follow_up`. That is test-only policy today.

The product continuous listener loop stays off until `Settings.listener_enabled`
is true (`[listener] enabled` in profile or `KURO_LISTENER=1`). PTT remains the
default path.