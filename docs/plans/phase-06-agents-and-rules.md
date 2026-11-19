# Phase 06: AGENTS.md and Cursor rules

- Size: S
- Depends on: phase 05
- Behavior change: none. Agent entry docs and scoped rules only.
- ADRs: none required unless editing protected paths already covered.
- Protected paths: none new.

## Goal

Give agents a single entry point and short scoped rules that match today's tree
(pre-scaffolders), so later phases do not invent conflicting instructions.

## Scope

In: Expand `AGENTS.md` from phase 01 v0 toward the final template in file 02
section 1, listing only commands that exist. Add `.cursor/rules/*.mdc` for core,
features (tools path today), router, engine, shell, ipc.

Out: Full scaffolder commands (phase 11), generated REPO_MAP (phase 11), turn.mdc
until `kuro-turn` exists (add stub note).

## Context card

Read:
- `docs/plans/02-guardrails.md` sections 1 and 2
- Existing `AGENTS.md` from phase 01
- Current crate layout under `kuro-rs/crates/`

Create:
- `.cursor/rules/00-core.mdc`
- `.cursor/rules/router.mdc`
- `.cursor/rules/tools.mdc` (globs `kuro-tools/**`; rename to features.mdc in phase 09)
- `.cursor/rules/engine.mdc`
- `.cursor/rules/shell.mdc`
- `.cursor/rules/ipc.mdc`

Edit: `AGENTS.md` (where-to-work table for current paths: `kuro-tools`, not yet
`kuro-features`).

Do not touch: product source, gate scripts.

Commands:
```powershell
cargo xtask gate fast
```

## Steps

1. Update AGENTS.md hard rules to match file 02; drop commands that do not exist yet
   (`new-feature`, `kuro-cli route`) with a "coming in phase N" note or omit.
2. Write each `.mdc` with front matter and at most 15 rule lines.
3. Point "read REPO_MAP first" to `docs/plans/README.md` until REPO_MAP is generated.
4. Document protected paths and ADR trailer requirement.

## Acceptance

- AGENTS.md lists only working commands.
- Rules load for matching globs; 00-core always applies.
- Gate fast green.

## Risks

- Rules that reference future crates confuse agents: use today's paths and note
  renames in comments only where needed.
