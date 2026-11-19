# ADR 0012: Gate tiers

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 8; findings C1, N24; phase 01

## Context

There is one gate, `scripts/Test-KuroGate.ps1`. It force-stops `KuroShell` and
`kuro-engine`, deletes `kuro.lock`, builds release, runs every test, builds the
shell, starts a live engine, and restarts the user's Kuro at the end. The pre-commit
hook runs all of it. Tests that need models return early when the model is missing,
so a green run proves less than it appears. The owner wants a non-destructive gate
that is fast locally (about 60 seconds) and complete in CI, with slow hardware tests
opt-in.

## Options

1. Keep one gate and make it faster.
2. Three tiers with different triggers.

## Decision

Option 2, implemented in `cargo xtask gate`.

- fast (pre-commit): text and size checks, protected-path check on the staged diff,
  `cargo fmt --check`, clippy on the debug profile, architecture tests, unit tests
  and the corpus for changed crates, and `KuroShell.Tests` only when shell files
  changed. No release build, no shell build, no live engine.
- full (pre-push and CI): everything for the whole workspace, shell build and tests,
  contract fixtures, generated-file freshness, and a live smoke on an isolated port
  and data directory.
- hardware (manual or scheduled): model-dependent tests, STT quality on fixture
  audio, device loopback, key injection, benchmarks.

Rules for all tiers: never stop, restart, or lock a running Kuro. Gate builds use
their own output directories. A test that needs a model or external tool declares
it; it is reported as skipped in fast and full and fails in hardware when missing.
The gate prints its duration and its skip count.

The 60 second budget is a warm-cache budget. A cold build of this workspace compiles
whisper.cpp and ONNX bindings and cannot meet it; phase 01 measures both numbers and
records them.

## Consequences

- Committing is cheap, so commits get smaller.
- A broken full gate is found at push time, not commit time. CI remains the
  authority.
- Separate output directories cost disk space and one extra cold build.
- Skips are visible, so "green" has a precise meaning per tier.

## Enforced by

The gate's own timing output; a CI assertion that the fast tier stays under 90
seconds warm; `scripts.mdc` (no `Stop-Process` on Kuro); a text check that fails if
`Stop-Process` and `kuro` appear together in `scripts/` or `xtask/`.
