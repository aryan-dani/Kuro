# Phase 18: Single supervisor

- Size: M
- Depends on: phase 17
- Behavior change: restart ownership consolidates in the shell. Watch-Kuro.ps1
  becomes thin or deprecated; App watchdog and shell supervisor no longer double
  restart.
- ADRs: 0010, 0012 (gate must not restart Kuro)
- Protected paths: supervisor code if listed. Trailer `ADR: 0010`.

## Goal

One supervisor owns the engine process lifecycle: policy in KuroShell.Core,
actuation in shell Services. Remove competing restart paths.

## Scope

In: Pure restart policy tests; shell supervisor service; thin Watch script or
delete; remove duplicate App watchdog restart; document runbook.

Out: Hub wiring (19); product listener mode (20).

## Context card

Read:
- `docs/adr/0010-single-supervisor.md`
- Findings on three restart paths (shell, App watchdog, Watch-Kuro.ps1)
- `scripts/Watch-Kuro.ps1` if present
- Gate scripts (must not start/stop Kuro; phase 01)

Create:
- `KuroShell.Core/Supervisor/` policy types + tests
- Runbook section in SUPPORT.md or docs/guides/runbook.md

Edit: shell Services supervisor; App.xaml.cs watchdog; Watch script.

Do not touch: hotkey; turn machine; gate kill behavior (already fixed in 01).

Commands:
```powershell
dotnet test KuroShell/KuroShell.Tests
cargo xtask gate full
```

## Tests first

- Policy: crash loop backoff, max restarts, clean exit no restart.
- Integration: killing engine once triggers one supervised restart (manual or
  automated carefully without gate interference).

## Steps

1. Implement pure policy.
2. Wire shell as sole actuator.
3. Disable/remove other restarters.
4. Update docs: how to run under supervision.

## Acceptance

- Only one component restarts the engine.
- Gate full never leaves a stray Kuro it started (phase 01 invariant).
- ADR 0010 enforced by test comments + CODEOWNERS if needed.

## Risks

- Dev workflow that relied on Watch-Kuro: document replacement.
