# Contributing

Kuro is a Windows desk voice agent (Rust engine + WinUI shell). Read
[AGENTS.md](AGENTS.md) and [ARCHITECTURE.md](ARCHITECTURE.md) first.

## Setup

```powershell
.\scripts\Build-KuroNative.ps1
.\scripts\Install-GitHooks.ps1
```

## Gate

```powershell
cargo xtask gate fast   # every commit
cargo xtask gate full   # every push
```

The gate must not kill or restart a running Kuro.

## PR rules

1. One phase file or one focused task per PR.
2. Tests first for behavior changes; characterization corpus for refactors.
3. Protected paths need an ADR and commit trailer `ADR: NNNN`.
4. No emoji, no em/en dashes, no personal data in tracked files.
5. Do not commit `.env`, tokens, or `%LOCALAPPDATA%\Kuro` contents.

## Tutorials

Feature scaffolder and the ten-minute "add a feature" guide land in phase 11.
Until then, add routing cases under `kuro-rs/fixtures/phrases/` and keep
`cargo test -p kuro-router --test corpus` green.
