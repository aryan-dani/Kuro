# ADR 0014: Bench tooling in xtask

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 9; findings 13, 21; phases 05, 21

## Context

`scripts/bench/*.py` import modules from the deleted `legacy/python`, so they cannot
run. `bench/` and the perf docs hold Python-era numbers from one personal machine.
The engine's own `timing.csv` records `tts_first_audio` as 0.0 and skips follow-up
turns. The performance figures in the task description (STT 350 ms warm, Kokoro
first audio 200 ms, router 2.4 ms, Ollama 1.8 s p50) cannot be reproduced from the
repository.

## Options

1. Repair the Python scripts. Keeps a second runtime the invariants forbid.
2. Delete everything and rely on `timing.csv`.
3. Delete the Python, add a small Rust benchmark runner, and fix `timing.csv`.

## Decision

Option 3.

- Delete `scripts/bench/*.py`, `bench/`, and the perf docs. Port
  `gen_kokoro_vocab.py` to `cargo xtask gen kokoro-vocab`.
- `cargo xtask bench <router|stt|tts|turn>` runs a fixed workload (the fixture audio
  and corpus), prints a table, and writes JSON to `kuro-rs/target/bench/`.
  - `router`: decide time per corpus case, exact and semantic.
  - `stt`: warm transcribe time per clip and real-time factor.
  - `tts`: first audio and total synthesis per sentence for each engine present.
  - `turn`: an end-to-end headless turn with real STT and TTS and a fake LLM.
- No benchmark framework is added; the runner uses `std::time::Instant`, medians,
  and p95 over a fixed iteration count.
- Results are machine-specific and never committed. Regression detection is a local
  ratchet file per machine (`%LOCALAPPDATA%\Kuro\bench-baseline.json`): the hardware
  tier fails when a median regresses by more than 15 percent against that machine's
  baseline.
- Production timing (`timing.csv`) is the second source of truth and is repaired in
  phase 19.

## Consequences

- One runtime. Benchmarks share fixtures with tests.
- Published numbers in docs must come from a named command and machine, or not
  appear.
- No historical continuity with the Python-era numbers, which were not comparable
  anyway.

## Enforced by

The invariant that only `scripts/build_icons.py` may be Python; the stale-term check
on docs; `xtask` unit tests for the statistics helpers.
