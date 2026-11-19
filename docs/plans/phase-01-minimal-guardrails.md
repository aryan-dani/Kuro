# Phase 01: Minimal guardrails and gate tiers

- Size: M (two parts, one PR each)
- Depends on: phase 00
- Behavior change: none in the product. Developer workflow: the gate stops killing
  and restarting Kuro.
- ADRs: 0012, 0018, 0001
- Protected paths touched: `scripts/Test-KuroGate.ps1`,
  `scripts/Test-KuroInvariants.ps1`, `.githooks/**`, `.github/**`. Use trailers
  `ADR: 0012` and `ADR: 0018`.

## Goal

Give every later phase a fast, non-destructive gate and the first mechanical rules,
before any product code moves.

## Scope

In:
- Part A: `xtask` with `check size`, `check text`, `check protected`; em dash cleanup;
  `AGENTS.md` v0; toolchain pin.
- Part B: `gate fast` and `gate full`; hooks; Linux and protected CI jobs; explicit
  skip accounting.

Out: scaffolders, repo map, architecture dependency rules (phase 11), CODEOWNERS and
templates (phase 05), porting the remaining invariants out of PowerShell.

## Context card

Read:
- `docs/plans/02-guardrails.md` sections 1, 3, 6, 8
- `docs/plans/03-tests-and-ci.md` sections 9 and 11
- `scripts/Test-KuroGate.ps1` (stages 1 to 6, `Stop-LockingKuro`, the final restart
  block)
- `scripts/Test-KuroInvariants.ps1`
- `.github/workflows/ci.yml`, `.githooks/pre-commit`, `kuro-rs/.cargo/config.toml`

Create:
- `kuro-rs/xtask/` (`Cargo.toml`, `src/main.rs`, `src/check/{size,text,protected}.rs`,
  `src/gate.rs`, `data/size_allowlist.toml`, `data/protected_paths.txt`,
  `data/portable_crates.txt`, `README.md`)
- `.cargo/config.toml` at the repo root (alias only)
- `kuro-rs/rust-toolchain.toml`
- `AGENTS.md`
- `.githooks/pre-push`
- `kuro-rs/crates/kuro-core/src/testkit/mod.rs`, `require.rs` (feature `testkit`)

Edit: `kuro-rs/Cargo.toml` (workspace member), `kuro-rs/.cargo/config.toml` (alias),
the two gate scripts, `scripts/Install-GitHooks.ps1`, `.githooks/pre-commit`,
`.github/workflows/ci.yml`, files containing em dashes, the three tests that skip
silently (`kuro-tts/src/kokoro.rs` tests, `kuro-tts/tests/cache.rs`,
`kuro-router/tests/semantic_router.rs`).

Do not touch: product logic in any `src/` file (comment and string-literal dash
replacement only), `KuroShell/` code.

Commands:
```powershell
cargo xtask check size
cargo xtask check text
cargo xtask gate fast
cargo xtask gate full
```

## Part A: checks, cleanup, AGENTS.md

Tests first (in `xtask`): size rules (new file over 400 fails, listed file may not
grow, listed file under the cap asks for removal); text rules (emoji, U+2014,
U+2013 fail with path and line); protected rule (changed protected file without an
`ADR:` trailer fails; with a trailer and an existing ADR file passes), using a temp
git repository.

Steps:
1. `xtask` has no dependency on product crates. Parse arguments by hand. Reuse
   workspace `toml` and `serde`.
2. `size_allowlist.toml` baseline: the 11 entries in file 02 section 6. Count
   physical lines.
3. Replace every em dash and en dash in tracked text. In
   `kuro-tts/src/kokoro_vocab.rs` use the escape `\u{2014}`; run the Kokoro tokenizer
   tests to prove the vocabulary is unchanged.
4. `protected_paths.txt`: the "protected today" list in file 02 section 3.
5. `AGENTS.md` from the template in file 02 section 1, listing only commands that
   exist after this phase, and a "where to work" table for the current layout.
6. `rust-toolchain.toml`: the stable version CI resolves today, plus `rustfmt`,
   `clippy`.
7. `Test-KuroInvariants.ps1` calls `cargo xtask check size` and `check text` and
   keeps its other checks.

## Part B: gate tiers and CI

Tests first: `xtask` unit tests for changed-crate selection from a list of changed
paths; a test that the fast tier command list contains no release build.

Steps:
1. `gate fast`: checks from part A, `cargo fmt --check`, `cargo clippy` (debug) on
   the workspace, tests for crates with changed files plus the corpus, and
   `dotnet test` for `KuroShell.Tests` only when `KuroShell/` changed. Print total
   seconds and the count of skipped model tests.
2. `gate full`: fast for all crates, `cargo test --workspace`, shell build and tests,
   contract snapshot, live smoke. Build with `--target-dir kuro-rs/target/gate` and
   `dotnet build --artifacts-path` so a running Kuro is never locked.
3. Delete `Stop-LockingKuro`, the `BUILDING` flag handling, and the final restart
   block from `Test-KuroGate.ps1`. The script becomes a wrapper for `gate full`.
4. `pre-commit` runs `gate fast`; new `pre-push` runs `gate full`.
5. `require_model!(name)` and `require_tool!(name)` in `kuro_core::testkit`: skip and
   print `SKIP <test> needs <name>` unless `KURO_HW_TESTS=1`, in which case fail.
   Replace the three silent early returns.
6. CI: keep the Windows job (calls `gate full`); add `portable` on `ubuntu-latest`
   running `cargo xtask check size`, `check text`, and `cargo test -p` for each crate
   in `portable_crates.txt`; add `protected` for pull requests. Determine the initial
   crate list by trying each crate on Linux; expect a short list. Set `CFLAGS` and
   `CXXFLAGS` explicitly in that job (finding N17).
7. Measure the fast tier warm and cold, locally and in CI; append the numbers to
   `docs/plans/baseline-measurements.md`.

## Acceptance criteria

- `rg` finds no U+2014 or U+2013 in tracked text files.
- Adding one line to `kuro-router/src/phrases.rs` makes `check size` fail.
- `gate fast` finishes under 60 s warm on the owner's machine, or the measured
  number and the largest contributor are reported for a decision.
- Running either gate tier while Kuro is running leaves the same process ids alive.
- CI shows three jobs green; the Linux job lists the crates it tested.
- The corpus and all snapshots from phase 00 are unchanged.

## Rollback

Revert the PR. The old gate script is restored by the revert.

## Guardrails

- No product behavior change. If a dash sits inside a string literal that is spoken
  or matched, stop and report it instead of editing.
- Never add `Stop-Process` for Kuro to any script.
- Keep `xtask/src/*.rs` files under 200 lines.
