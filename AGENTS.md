# AGENTS.md

Kuro is a Windows desk voice agent: Rust engine in `kuro-rs/`, WinUI shell in
`KuroShell/`. Read `docs/REPO_MAP.md` first (run `cargo xtask gen` if it is missing),
then the README of the folder you will edit, then code.

## Commands

- Before every commit: `cargo xtask gate fast`
- Before every push: `cargo xtask gate full`
- Mechanical checks: `cargo xtask check size`, `cargo xtask check text`,
  `cargo xtask check arch`, `cargo xtask check protected --base <ref>`
- Regenerate repo map: `cargo xtask gen` (staleness: `cargo xtask gen check`)
- Scaffold: `cargo xtask new-feature <id>`, `cargo xtask new-adr "<title>"`
- CLI: `cargo run -p kuro-cli -- init --defaults`, `doctor`, `doctor --json`
- Coming later: `cargo run -p kuro-cli -- route --explain`, scaffolders
  (`new-adapter`, `new-setting`, `new-shell-page`, `new-ritual`)

## Where to work

| Task | Edit here | Never edit for this |
|------|-----------|---------------------|
| Voice feature | `kuro-rs/crates/kuro-features/src/features/<id>/` | engine loop, router |
| Phrase or routing | `kuro-rs/crates/kuro-router/src/` or `kuro-rs/fixtures/phrases/` | tool bodies, shell |
| Engine loop, API, HUD | `kuro-rs/crates/kuro-engine/src/` | tool implementations |
| IPC, handshake, `/say` | `kuro-rs/crates/kuro-ipc/` | tool logic |
| Hotkey | `kuro-rs/crates/kuro-hotkey/` | shell |
| TTS, STT | `kuro-rs/crates/kuro-tts/`, `kuro-stt/` | router |
| Audio, Wi-Fi helpers | `kuro-rs/crates/kuro-audio/`, `kuro-wifi/` | features |
| Shared core | `kuro-rs/crates/kuro-core/` | adapter-specific code |
| Shell UI | `KuroShell/KuroShell/` | engine hotkey registration |
| Gate and checks | `scripts/`, `kuro-rs/xtask/` | product behavior in `src/` |

## Hard rules

1. Run `cargo xtask gate fast` before finishing a change.
2. Tests first. A behavior change needs a failing test in the same PR.
3. A refactor PR changes no behavior. Router golden corpus files must not change in it.
4. PROTECTED paths (see `kuro-rs/xtask/data/protected_paths.txt` and
   `.github/CODEOWNERS`) need `docs/adr/NNNN-*.md` and commit trailer `ADR: NNNN`.
5. No emoji, no em dashes (U+2014), no en dashes (U+2013) in tracked text files.
6. No personal data (names, IPs, hostnames, user paths) in tracked files.
7. Never kill or restart a running Kuro from a script or test.
8. Files: soft target 200 lines, cap 400 unless allowlisted in
   `kuro-rs/xtask/data/size_allowlist.toml`.
9. One phase file or one task per PR. If the task text and the code disagree, the
   code wins: stop and report.

## Invariants

- Ctrl+Alt+Space hold-to-talk is owned by the engine, not the shell.
- The HTTP API stays on 127.0.0.1 with `X-Kuro-Token`.
- Dismiss and Escape cancel through the turn epoch (see `kuro-core/src/epoch.rs`).
- Tool results reach the user as speakable strings.

## When unsure

Ask instead of guessing for hardware behavior, licensing, anything marked
"unverified", and any edit to a PROTECTED path.

## Cursor rules

Scoped rules live in `.cursor/rules/`. `turn.mdc` arrives with `kuro-turn` (phase 12).
`features.mdc` covers `kuro-features`. `tools.mdc` remains for legacy `kuro-tools` until removal.