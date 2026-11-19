# Phase 16: HUD and capsule split

- Size: M
- Depends on: phase 15
- Behavior change: none. File and folder splits only. Capsule remains
  non-activating and must not move on state change.
- ADRs: 0002 (budgets applied to C# where practical)
- Protected paths: capsule placement if listed. Trailer if needed.

## Goal

Split Capsule and Hud into focused files (animation, rendering, placement, state
binding) so HUD animation changes touch one file. Preserve invariants from AGENTS.md.

## Scope

In: Folder layout under `KuroShell/KuroShell/Capsule/` and `Hud/`; READMEs;
move code per split plan; shell tests for placement and non-activation.

Out: MainWindow page host (17); supervisor (18).

## Context card

Read:
- `docs/plans/01-split-plans.md` (shell HUD/capsule)
- Capsule and Hud sources; `HUD_DESIGN.md` if present
- Existing KuroShell.Tests

Create: animation, spring binding, transcript view modules as planned;
`Capsule/README.md`, `Hud/README.md`.

Edit: project file includes; oversized cs files.

Do not touch: engine; Pages content; tray supervisor logic beyond compiles.

Commands:
```powershell
dotnet test KuroShell/KuroShell.Tests
cargo xtask gate fast
```

## Tests first

- Placement invariant tests (no move on state).
- Bar count / visual contract tests if present stay green.

## Steps

1. Extract pure motion helpers into KuroShell.Core/Motion if not already.
2. Split UI files; keep public surface for MainWindow.
3. Document "change HUD animation" path in README (agent-ability row).

## Acceptance

- Invariants hold; tests green.
- Animation edit path matches AGENTS table.

## Risks

- WinUI partial class wiring breakage: split carefully with partials.
