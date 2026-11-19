# 02 Guardrails

Goal: make the wrong change fail mechanically. Prose rules are the second layer.

Order of introduction: phase 01 lands the minimum (AGENTS.md v0, size ratchet, text
checks, fast gate). Phase 05 adds CODEOWNERS and templates. Phase 11 adds scaffolders,
the generated map, and the full architecture tests. Rule files grow with the tree.

## 1. `AGENTS.md` (repo root)

Final content (phase 11). Phase 01 ships the same file with only the commands that
exist at that point.

````markdown
# AGENTS.md

Kuro is a Windows desk voice agent: Rust engine in `kuro-rs/`, WinUI shell in
`KuroShell/`. Read `docs/REPO_MAP.md` first, then the README of the folder you will
edit, then code.

## Commands
- Before every commit: `cargo xtask gate fast`
- Before every push: `cargo xtask gate full`
- Regenerate map, config docs, generated code: `cargo xtask gen`
- Explain a routing decision: `cargo run -p kuro-cli -- route --explain "<text>"`
- Scaffold: `cargo xtask new-feature <id>`, `new-adapter <kind> <id>`,
  `new-setting <section> <key>`, `new-shell-page <Name>`, `new-ritual <id>`,
  `new-adr "<title>"`

## Where to work
| Task | Edit here | Never edit for this |
|------|-----------|---------------------|
| Voice feature | `kuro-rs/crates/kuro-features/src/features/<id>/` | engine, router, turn |
| Phrase or reply | the feature's `feature.toml` | Rust code |
| Setting | `[[settings]]` in `feature.toml` or `kuro-config/sections/` | generated files |
| Routing bug | `kuro-router/src/` or the feature manifest | feature execute code |
| TTS, STT, LLM, trigger adapter | that crate's `engines/` or `providers/` | `kuro-turn` |
| Shell page | `KuroShell/KuroShell/Pages/<Name>/` | `MainWindow.xaml` |
| HUD animation | `Capsule/` or `Hud/` animation files | `CapsulePlacement.cs` |

## Hard rules
1. Tests first. A behavior change needs a failing test in the same PR.
2. A refactor PR changes no behavior. The golden corpus must not change in it.
3. PROTECTED paths (see `.github/CODEOWNERS`) need an ADR and the commit trailer
   `ADR: NNNN`.
4. No statics, no `thread_local!`, no `std::env::var` outside `kuro-config`, no
   `ureq` outside the HTTP adapter, no `thread::spawn` inside features.
5. No `unwrap` or `expect` on anything that comes from outside the process.
6. Features return `Result<Outcome, FeatureError>`. Spoken text lives in TOML.
7. Files: target under 200 lines, cap 400. Functions under about 60 lines.
8. No emoji. No em dashes. No personal data (names, IPs, hostnames, user paths).
9. Secrets only in `%LOCALAPPDATA%\Kuro` or ignored files.
10. Never edit generated files. Run `cargo xtask gen`.
11. Never kill or restart a running Kuro from a script or test.
12. One phase file or one task per PR. If the task text and the code disagree, the
    code wins: stop and report.

## Invariants
- Ctrl+Alt+Space hold-to-talk is owned by the engine.
- The capsule is non-activating and never moves on state change.
- The API stays on 127.0.0.1 with `X-Kuro-Token`.
- Dismiss and Escape cancel through the turn epoch.
- Tool results reach the user as speakable strings.

## When unsure
Ask instead of guessing: hardware behavior, licensing, anything marked
"unverified", and any edit to a PROTECTED path.
````

## 2. `.cursor/rules/*.mdc`

Each file has front matter (`description`, `globs`, `alwaysApply`) and at most 15
lines of rules. Exact contents:

| File | Globs | Rules |
|------|-------|-------|
| `00-core.mdc` | always | Read `AGENTS.md`. Run the fast gate before finishing. No emoji, no em dashes, no personal data. Generated files are read-only. Report disagreements between task and code. |
| `features.mdc` | `kuro-rs/crates/kuro-features/**` | Stay inside one feature folder. Phrases, replies, settings are data in `feature.toml`. Use `ctx.http`, `ctx.clock`, `ctx.store`; never statics, env, or raw threads. Every error path has a test in `tests.rs` using fakes. Update `README.md` and `routing_cases.toml`. |
| `router.mdc` | `kuro-rs/crates/kuro-router/**`, `kuro-rs/fixtures/phrases/**` | Add the failing corpus case first. The router never executes anything. Changing a threshold or stage order requires a corpus diff in the PR description. |
| `turn.mdc` | `kuro-rs/crates/kuro-turn/**`, `kuro-rs/crates/kuro-core/src/epoch.rs` | PROTECTED. `step` is pure: no I/O, no clock reads, no ports. Every new transition needs a scenario test. Cancel must reach Idle from every state. |
| `adapters.mdc` | `kuro-rs/crates/kuro-{audio,stt,tts,llm,triggers,wifi}/**` | Implement the port trait only. No dependency on another adapter crate. Register in the crate's own `registry.rs`. Model paths come from config. Tests must pass with no model present and say so explicitly. |
| `protected.mdc` | hotkey, protocol, `contract/**`, token, auth, supervisor, `CapsulePlacement.cs`, `.github/**` | Stop. This path is PROTECTED. Do not edit without an ADR number from the user. Use the trailer `ADR: NNNN`. |
| `shell.mdc` | `KuroShell/**` | Logic goes in `KuroShell.Core` with tests. Pages register in `PageRegistry.cs`. The shell never registers a global hotkey. Only `CapsulePlacement.cs` positions the capsule. No `async void` except through the helper. DTO changes start in `contract/`. |
| `scripts.mdc` | `scripts/**`, `kuro-rs/xtask/**`, `.githooks/**` | Scripts are thin wrappers over `cargo xtask`. Never `Stop-Process` Kuro. Never write outside the repo or a temp directory in checks. |
| `tests.mdc` | `**/tests/**`, `**/tests.rs`, `kuro-rs/fixtures/**` | Deterministic: no sleep, no network, no env mutation, no real devices, no wall clock. Use `kuro_core::testkit` fakes. Golden files change only with `KURO_BLESS=1` and a reviewed diff. |
| `docs.mdc` | `docs/**`, `**/*.md` | No em dashes, no emoji. `docs/REPO_MAP.md`, `docs/CONFIG.md`, `docs/features/**` are generated. Folder `README.md` files stay under 60 lines. |

## 3. CODEOWNERS and PROTECTED paths

Single source: `kuro-rs/xtask/data/protected_paths.txt` (globs, one per line).
`.github/CODEOWNERS` is generated from it by
`cargo xtask gen codeowners --owner <handle>`.

Protected today (phase 01 list):

```
kuro-rs/crates/kuro-hotkey/**
kuro-rs/crates/kuro-core/src/epoch.rs
kuro-rs/crates/kuro-core/src/token.rs
kuro-rs/crates/kuro-ipc/src/lib.rs
KuroShell/KuroShell/EngineSupervisor.cs
KuroShell/KuroShell/EngineJson.cs
KuroShell/KuroShell/EscapeHook.cs
scripts/Test-KuroGate.ps1
scripts/Test-KuroInvariants.ps1
.githooks/**
.github/**
kuro-rs/xtask/data/**
```

Added as the tree changes: `kuro-rs/crates/kuro-protocol/**`, `contract/**`,
`kuro-rs/crates/kuro-turn/src/machine/**`, `kuro-rs/crates/kuro-config/src/secrets.rs`,
`kuro-rs/crates/kuro-ipc/src/auth.rs`, `kuro-rs/crates/kuro-engine/src/threads/**`,
`KuroShell/KuroShell.Core/Supervisor/**`,
`KuroShell/KuroShell/Capsule/CapsulePlacement.cs`.

Mechanical check: `cargo xtask check protected --base <ref>` lists changed files
between the base and HEAD. If any matches a protected glob, every commit touching it
must carry the trailer `ADR: NNNN` and `docs/adr/NNNN-*.md` must exist. It runs in the
fast gate against the staged diff and in CI against the PR range. Phases in this plan
that edit protected paths name the ADR to cite.

## 4. Scaffolders (`cargo xtask ...`, phase 11; `new-shell-page` in phase 17)

| Command | Creates | Registers | Then runs |
|---------|---------|-----------|-----------|
| `new-feature <id>` (alias `new-tool`) | `features/<id>/` with `README.md`, `feature.toml`, `mod.rs`, `tests.rs`, `routing_cases.toml`, `config.gen.rs` | two marked lines in `features/mod.rs` | `gen`, the new tests |
| `new-adapter <kind> <id>` | `<crate>/src/engines/<id>/` with `mod.rs`, `adapter.toml`, `tests.rs`, `README.md` | one line in that crate's `registry.rs` | `gen` |
| `new-setting <section> <key>` | a `[[settings]]` block | nothing | `gen` |
| `new-shell-page <Name>` | `Pages/<Name>/<Name>Page.xaml`, `.xaml.cs`, `KuroShell.Core/ViewModels/<Name>ViewModel.cs`, a test stub | one line in `PageRegistry.cs` | shell tests |
| `new-ritual <id>` | `profiles/example/rituals/<id>.toml` and a routing case | nothing (rituals are discovered as data) | profile validation |
| `new-hub-event <name>` | `features/hub/events/<name>.rs`, fixture JSON | one line in `hub/events/mod.rs` | hub tests |
| `new-adr "<title>"` | next numbered ADR from the template | index in `docs/adr/README.md` | nothing |

Every generated Rust and C# file starts with the header comment (purpose, public API,
invariants, must-not-depend) and compiles with one passing and one deliberately
pending test (`todo` text in an `#[ignore]` reason, never `todo!()`).

## 5. Architecture tests (`cargo xtask check arch`)

| # | Rule | How | Mode |
|---|------|-----|------|
| 1 | Crate dependency table (file 01 section 2) | `cargo metadata` | fail |
| 2 | Portable crates have no direct `windows`, `windows-sys`, `cpal`, `rodio`, `whisper-rs` dependency outside `cfg(windows)` | `cargo metadata` | fail |
| 3 | No `static` items with interior mutability and no `thread_local!` | source scan | ratchet |
| 4 | No `std::env::var` outside `kuro-config` | source scan | ratchet |
| 5 | No `ureq::` outside the HTTP adapter | source scan | ratchet |
| 6 | No `thread::spawn` in `kuro-features` | source scan | fail |
| 7 | `unwrap_used`, `expect_used` denied in `kuro-features` and adapters (tests exempt) | clippy | fail |
| 8 | Every folder in `features/` is registered and has `README.md`, `feature.toml`, `tests.rs` | file scan | fail |
| 9 | Manifest ids and priorities are unique; every reply key used in code exists | manifest load | fail |
| 10 | Personal-data patterns: CGNAT and private IPv4 literals, `C:\Users\<name>`, plus an optional untracked `personal_denylist.local.txt` | text scan | fail |
| 11 | Size ratchet (section 6) | line count | ratchet |
| 12 | Every source file starts with a header comment | text scan | ratchet |
| 13 | No emoji, no em dash (U+2014), no en dash (U+2013) in tracked text | text scan | fail |
| 14 | Generated outputs are fresh (`gen --check`) | regenerate and diff | fail |
| 15 | `clippy::too_many_lines` at 60 lines | clippy; count of `allow` attributes | ratchet |

"Ratchet" means the current count is frozen in `kuro-rs/xtask/data/ratchets.toml` and
may only go down. The check fails on an increase and asks for the file to be updated
on a decrease.

## 6. Size ratchet

`kuro-rs/xtask/data/size_allowlist.toml` maps path to physical line count.
- A hand-written `.rs`, `.cs`, `.ps1`, `.xaml` file over 400 lines that is not listed
  fails.
- A listed file may not exceed its recorded count.
- A listed file at or under 400 fails with "remove this entry".
- Files between 200 and 400 are reported, not failed.
- Folder `README.md` and `MAP.md` files over 60 lines fail.

Baseline entries (phase 01): `NativeCapsule.cs` 919, `semantic.rs` 704, `phrases.rs`
702, `kuro-engine/src/main.rs` 621, `HudWindow.cs` 602, `kokoro.rs` 568,
`scripts/bench/run_stt.py` 518, `MainWindow.xaml.cs` 506, `kuro-ipc/src/lib.rs` 474,
`docs/NATIVE_REWRITE_SPEC.md` 474, `wa_neonize.rs` 410.

## 7. Repo map generation

`cargo xtask gen` writes `docs/REPO_MAP.md` from: each crate or project `README.md`
first paragraph, each source file's first header line and line count, `lib.rs`
re-exports, and feature manifests (id, intents, settings). CI runs
`cargo xtask gen --check`. Per-folder `README.md` files are hand-written and capped at
60 lines: purpose, file list, entry points, "edit here to", "never edit here for".

## 8. Gate tiers (ADR 0012)

| Tier | Command | Runs | Budget | Where |
|------|---------|------|--------|-------|
| fast | `cargo xtask gate fast` | text and size checks, protected-path check on the staged diff, `cargo fmt --check`, `cargo clippy` (debug), arch tests, unit tests and corpus for changed crates, `dotnet test` on `KuroShell.Tests` only when `KuroShell/` changed | under 60 s warm | pre-commit |
| full | `cargo xtask gate full` | fast tier for the whole workspace, `cargo test --workspace`, shell build and tests, contract fixtures, `gen --check`, live engine smoke on an isolated port and data directory | under 10 min | pre-push, CI |
| hardware | `cargo xtask gate hardware` | model-dependent tests, STT WER on fixture audio, device loopback, bench | opt-in | manual, scheduled |

Rules: no tier kills, restarts, or locks a running Kuro. Builds for the gate use their
own output directories (`kuro-rs/target/gate`, `dotnet --artifacts-path`). A test that
needs a model calls `require_model!`; it is reported as skipped in fast and full and
fails in hardware. The gate prints the skip count.

## 9. Agent-ability test

Acceptance: at most 3 files read, no edits outside the target folder except generated
or registered files. "Today" counts are from the findings.

| Task | Today (files, areas) | Target: read | Target: edit | Registered or generated | Pass |
|------|----------------------|--------------|--------------|-------------------------|------|
| Add a tool | 9 to 11 files, 4 crates | `kuro-features/README.md`, generated `mod.rs`, `feature.toml` | `features/<id>/` | `features/mod.rs`, generated docs | yes |
| Add a phrase variant | 2 to 3 files of 700 lines | the feature's `feature.toml` | `feature.toml`, `routing_cases.toml` | none | yes |
| Add a setting | 6 to 8 files, 2 languages, and it has no effect | `feature.toml`, the code file that uses it | same two files | `config.gen.rs`, `CONFIG.md`, schema | yes |
| Add a shell page | 3 files, 230 to 506 lines | `Pages/README.md`, generated page, generated view model | `Pages/<Name>/`, the view model file | `PageRegistry.cs` | yes, two projects |
| Add a TTS adapter | 5 to 7 files, both languages | `kuro-tts/README.md`, `ports/tts.rs`, one existing engine | `engines/<id>/` | `kuro-tts/src/registry.rs` | yes |
| Add a hub event | not possible, hub is unwired | `features/hub/README.md`, one existing event file | `hub/events/<name>.rs`, fixture | `hub/events/mod.rs` | yes, after phase 19 |
| Fix a routing bug | 2 files of 700 lines plus 5 test files, no trace | output of `route --explain`, the one file it names, the corpus file | that file, one corpus case | none | yes |
| Change a HUD animation | 919 or 602 line file | `Capsule/README.md`, the animation file, `Motion/Spring.cs` | the animation file | none | yes |

Each row becomes a dry-run exercise at the end of the phase that enables it: a fresh
agent session gets only the task sentence and `AGENTS.md`, and the reviewer records
the files it opened. The phase is not done until the count is 3 or fewer.
