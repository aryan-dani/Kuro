# Phase 08: kuro init and doctor

- Size: M
- Depends on: phase 07
- Behavior change: new CLI commands; no change to running voice loop defaults when
  user does not run init.
- ADRs: 0005
- Protected paths: none.

## Goal

Ship a `kuro` CLI binary (`kuro-cli`) with `init` and `doctor` so a template clone
can create an example profile and diagnose missing models, voices, and permissions.

## Scope

In: `kuro-cli` crate; `kuro init [--defaults]`; `kuro doctor [--json]`; write
profile under user data or `profiles/local/`; check Rust/toolchain, models path,
WinRT voice, mic permission hints, API bind, token file presence.

Out: `route --explain` (can stub), full scaffolder integration, shell settings UI.

## Context card

Read:
- `docs/plans/04-repo-hygiene.md` README quickstart
- `docs/adr/0005-config-profiles-secrets.md`
- `kuro-config` load API from phase 07
- Existing binary entry in `kuro-engine` if any

Create:
- `kuro-rs/crates/kuro-cli/` (main, init, doctor modules)
- Doctor JSON schema documented in SUPPORT.md

Edit: workspace members; README quickstart to use `kuro init --defaults`;
SUPPORT.md doctor output section.

Do not touch: router, tools execute, shell UI.

Commands:
```powershell
cargo run -p kuro-cli -- doctor --json
cargo run -p kuro-cli -- init --defaults
cargo xtask gate fast
```

## Tests first

- `init --defaults` is idempotent; second run does not clobber without `--force`.
- Doctor returns non-zero when a required check fails; JSON always parses.
- Init never writes secrets into the git tree.

## Steps

1. Add `kuro-cli` depending on core, config, protocol (optional).
2. Implement doctor checks as pure functions over filesystem/env fakes in tests.
3. Init copies `profiles/example` to the user profile location.
4. Document in README: after clone, `cargo build -p kuro-cli && kuro init --defaults`.

## Acceptance

- New user path documented and works on a clean machine checklist.
- Doctor JSON usable in bug reports.
- Gate fast green; no corpus drift.

## Risks

- Path differences Windows vs CI: doctor checks that need WinRT skip on Linux with
  explicit skip reason (portable job).
