# ADR 0008: Turn state machine and ports

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 6; findings S3, 11, 13, 14, N4, N6, N7; phases 12, 13

## Context

A voice turn is a 535-line `main()` with closures, about 25 shared handles, a `busy`
flag, and a nested follow-up loop that only exists when the API is on. Cancel is
handled in three places, and `/dismiss` leaves `busy` set. Nothing can be tested
without a microphone, speakers, and models. The owner leans toward a state machine
behind traits `Mic`, `Stt`, `Tts`, `Router`, `Llm`, `Clock`, `HudSink`.

## Options

1. Extract functions from `main()` and inject trait objects into the existing worker
   thread. Testable, but control flow stays implicit and cancel stays scattered.
2. An async state machine on tokio. Natural for streaming, but it pulls the audio
   path into an async runtime and makes the hotkey thread interplay harder.
3. A pure step function with effects, driven by one thread and plain channels.

## Decision

Option 3.

- `kuro-turn` exposes `step(state, event) -> (state, effects)`. It performs no I/O
  and reads no clock.
- One driver thread executes effects through ports and feeds results back as events.
  Long effects (transcribe, execute, LLM, speak) run on worker threads and report
  back with the epoch they started under.
- Ports in `kuro-core/src/ports/`: the owner's seven, plus `Trigger` (activation
  source), `AudioOut` (one persistent output), and `FeatureRunner`. The router port
  decides only; it never executes.
- `TurnClock` keeps its meaning. Cancel is one event from any source (Escape,
  dismiss, API, barge-in) and reaches Idle from every state. Stale results are
  dropped by epoch. The `busy` flag is removed.
- The engine hook keeps owning Ctrl+Alt+Space. The shell's Escape hook only forwards
  a dismiss request; its timing heuristics are removed once the machine is in place.
- Follow-up policy is data (`always`, `question`, `never`), default unchanged until
  phase 20.
- Headless tests use fakes from `kuro_core::testkit` and a scenario builder.

## Consequences

- Every transition is a table row with a test; the table is the specification.
- Timing marks come from effect boundaries, so `tts_first_audio` is real and
  follow-up turns are measured.
- Streaming LLM to TTS becomes a series of `LlmSentence` events with no new
  mechanism.
- The move is the riskiest refactor in the plan; it lands only after the corpus,
  scenario tests written against the old behavior, and the feature migration.

## Enforced by

Scenario tests in `kuro-turn/tests/`; the property "Cancel reaches Idle from any
state"; `turn.mdc`; PROTECTED status of `kuro-turn/src/machine/` and `epoch.rs`.
