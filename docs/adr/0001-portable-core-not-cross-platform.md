# ADR 0001: Windows product with a portable core

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 1; findings N17, N23; phases 01, 09, 12

## Context

Kuro's value depends on Windows pieces: a low-level keyboard hook, a layered
non-activating WinUI capsule, WinRT speech, WLAN APIs, the named pipe. The owner
leans toward ports so core crates build and test on Linux and asked whether the
template should claim "cross-platform" or "portable core".

Today even `kuro-core` depends on `windows-sys` and `ort`, and
`kuro-rs/.cargo/config.toml` injects MSVC compiler flags for every target.

## Options

1. Claim cross-platform. Requires a second shell, hotkey, audio, and TTS stack. None
   exists and none is planned.
2. Windows only, no portability effort. Cheapest, but logic stays untestable without
   Windows and architecture rot has no tripwire.
3. Windows product, portable core. Logic crates build and test on Linux in CI as an
   architecture check. No non-Windows adapter is promised.

## Decision

Option 3. The README says: "Kuro runs on Windows. Its core logic is portable and is
tested on Linux in CI." The portable set is `kuro-core`, `kuro-config`,
`kuro-protocol`, `kuro-router`, `kuro-llm`, `kuro-features` (logic), `kuro-turn`, and
`xtask`. The Linux job starts with the crates that build there today and the list
only grows.

## Consequences

- The Linux job is a cheap, fast proof that logic does not depend on hardware or
  Windows APIs.
- Windows-specific code inside portable crates must sit in `win.rs` files behind
  `cfg(windows)` with a trait for fakes.
- Nobody may read the Linux job as a promise of a Linux or macOS release.
- The MSVC `[env]` flags must stop applying to non-Windows targets.

## Enforced by

The `portable` CI job; architecture rules 1 and 2; the crate list ratchet in
`kuro-rs/xtask/data/portable_crates.txt`.
