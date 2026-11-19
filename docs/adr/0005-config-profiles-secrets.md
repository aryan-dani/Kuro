# ADR 0005: Config, profiles, and secrets

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 4; mandate 5c; findings 9, N1, N10, N21; phases 07, 08

## Context

Settings are scattered: `EngineConfig` reads env at start, `load_env_file` has a
13-key allowlist that omits real keys, the shell forces env at spawn, three different
default models exist, and `PUT /config` changes nothing the voice path reads and is
never persisted. The owner leans toward one typed validated config with defaults,
profile file, env, API overrides, a JSON schema, and a generated `.env.example`.

## Options

1. One hand-written struct with `schemars` and serde. Simple to read, but every
   feature must edit a central file, and env aliases, secrecy, and restart flags need
   a second table that can drift.
2. Settings declared as data beside their owner (`[[settings]]` in each
   `feature.toml` and in `kuro-config/sections/*.toml`), one loader, generated typed
   accessors.
3. Stringly typed lookups at run time. No generation, but typos become run-time bugs.

## Decision

Option 2.

- Declaration: key, type, default, range or enum, doc, `env` alias, `secret`,
  `restart`. `cargo xtask gen` writes `config.gen.rs` beside each declaration,
  `docs/CONFIG.md`, `contract/v1/config.schema.json`, and `.env.example`.
- Primary surface: TOML. Precedence: process env, then
  `%LOCALAPPDATA%\Kuro\config.toml`, then the profile, then defaults. A key pinned by
  env is reported as `locked`.
- API overrides are written to `config.toml`, so they survive restarts, and they take
  effect because the turn machine reads the live config.
- Secrets: only in `%LOCALAPPDATA%\Kuro\secrets.toml` or env. Never in `config.toml`,
  the repo, logs, or `/config` responses. `.env.example` lists secrets and developer
  overrides only.
- Profile: `profiles/example/` is tracked and harmless. A user profile lives at
  `%LOCALAPPDATA%\Kuro\profile\` or at `KURO_PROFILE_DIR` (which may be a private
  repository). It holds persona, extra phrases, corrections, rituals, contacts,
  workflows, and the profile's own routing tests.
- Every feature has `enabled` (the kill-switch), declared automatically.
- The shell stops forcing env at spawn; it passes nothing but the data directory.
- `kuro init` writes these files; `kuro doctor` validates them and can copy an old
  layout forward (`--migrate`, copy only).

No `schemars`: the schema is generated from the same declarations.

## Consequences

- "Add a setting" is one block of TOML and one use site.
- Generated Rust is checked in and must be fresh in CI.
- Legacy env names keep working through `env` aliases.
- An invalid file stops the engine with exit code 64 and a readable message instead
  of silently using defaults.

## Enforced by

Architecture rules 4 and 14; `config::tests` (precedence, validation, legacy layout);
the agent-ability row "add a setting"; gitleaks and the secret scan.
