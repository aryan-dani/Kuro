# Phase 07: Typed config, profiles, secrets

- Size: M
- Depends on: phase 06
- Behavior change: configuration loading path changes; default values must match
  today's env/defaults. No phrase or tool behavior change when env is unset.
- ADRs: 0005
- Protected paths: none unless touching hotkey/config that is protected. Trailer
  `ADR: 0005` if needed.

## Goal

Introduce `kuro-config` as the only crate that reads environment variables and
files; load a typed settings tree; support a gitignored user profile overlay and
secrets outside the repo.

## Scope

In: New crate `kuro-config`; `Settings` struct mirroring current knobs; load order
(defaults < example profile < user profile < env); move `std::env::var` call sites
out of other crates behind a thin compatibility shim if needed; `profiles/example/`;
gitignore for `profiles/local/` and `%LOCALAPPDATA%\Kuro` docs.

Out: `kuro init` / `kuro doctor` CLIs (phase 08); feature.toml settings merge
(phase 09); full CONFIG.md generator (phase 11).

## Context card

Read:
- `docs/adr/0005-config-profiles-secrets.md`
- `docs/plans/01-architecture.md` (config section if present)
- All `std::env::var` / `env::var_os` usages under `kuro-rs/crates/`
- Current defaults in engine main, ipc, tools

Create:
- `kuro-rs/crates/kuro-config/` (Cargo.toml, lib, load, settings modules)
- `profiles/example/config.toml` (harmless defaults)
- `profiles/example/README.md`
- `.gitignore` entries for local profiles and secret paths

Edit: workspace `Cargo.toml` members; crates that read env to take `&Settings` or
keep a deprecated env bridge behind a single function in `kuro-config`.

Do not touch: routing logic, tool execute bodies beyond config plumbing, shell.

Commands:
```powershell
cargo test -p kuro-config
cargo xtask gate fast
```

## Tests first

- Load defaults with no files and no env: equals today's documented defaults.
- Example profile overrides one key; env overrides profile.
- Missing optional file is ok; invalid TOML fails with typed error.
- Secrets path is never read from the repo tree.

## Steps

1. Inventory every env key (file 04 / findings). Map to typed fields.
2. Implement load order and validation.
3. Wire engine and ipc to `Settings::load()`.
4. Keep behavior: when only defaults apply, corpus and existing tests pass unchanged.
5. Document keys in a short `docs/CONFIG.md` stub (full gen later).

## Acceptance

- No `std::env::var` outside `kuro-config` (enforced later by arch check; for now
  grep clean or allowlisted shim).
- Defaults preserve behavior; gate + corpus green.
- Example profile tracked; local profile gitignored.

## Risks

- Subtle default mismatch: freeze defaults with unit tests copied from current code.
- Circular deps: `kuro-config` depends only on `kuro-core`.
