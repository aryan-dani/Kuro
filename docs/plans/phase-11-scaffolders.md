# Phase 11: Scaffolders, map, arch checks

- Size: M
- Depends on: phase 10
- Behavior change: none. Developer tooling only.
- ADRs: 0002, 0003, 0012
- Protected paths: possibly CODEOWNERS generation. Trailers as needed.

## Goal

Add `cargo xtask new-feature`, `new-adapter`, `new-setting`, `new-shell-page`,
`new-ritual`, `new-adr`; `cargo xtask gen` for REPO_MAP and CONFIG; `check arch`
for crate layering; finalize AGENTS.md commands.

## Scope

In: xtask scaffolder modules; templates under `xtask/templates/`; generated
`docs/REPO_MAP.md`, `docs/CONFIG.md`; `check arch` from cargo metadata; tutorial
section in CONTRIBUTING; cursor rules updated for kuro-features.

Out: Bench (can stub per ADR 0014); full shell page host (17).

## Context card

Read:
- `docs/plans/02-guardrails.md` sections 1, 5, 9
- `docs/adr/0002-crate-layering-and-size-budgets.md`
- Existing `kuro-rs/xtask/` from phase 01
- Sample feature folder from phase 10

Create:
- `xtask/src/new_feature.rs` (and siblings)
- `xtask/templates/feature/**`
- `xtask/src/gen/{repo_map,config_docs}.rs`
- `xtask/src/check/arch.rs`
- `docs/REPO_MAP.md` (generated)
- `docs/CONFIG.md` (generated)
- `docs/guides/add-a-feature.md` (complete tutorial)

Edit: AGENTS.md command list; CONTRIBUTING tutorial; features.mdc globs.

Do not touch: product runtime behavior.

Commands:
```powershell
cargo xtask new-feature sample_demo
cargo xtask gen
cargo xtask check arch
cargo xtask gate fast
```

## Tests first

- Scaffolder dry-run creates expected paths and two registry lines.
- Arch check fails on a synthetic forbidden edge.
- Gen is idempotent.

## Steps

1. Implement new-feature writing folder + patching features/mod.rs.
2. Implement gen readers over crate tree and settings schema.
3. Arch check allowlist portable crates for Linux CI.
4. Update AGENTS.md to final command set from file 02.

## Acceptance

- Agent can add a feature with scaffolder only; tutorial under 10 minutes.
- `check arch` in gate fast.
- REPO_MAP committed or generated in CI (pick one; prefer commit after gen).

## Risks

- Fragile mod.rs patching: use markers `// scaffold:register` comments.
