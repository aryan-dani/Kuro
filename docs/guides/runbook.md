# Kuro runbook (supervision)

## Who supervises the engine

The WinUI shell (`KuroShell`) is the only restart authority for `kuro-engine`.
Restart backoff, crash-loop limits, and exit-code rules live in
`KuroShell.Core/Supervisor/RestartPolicy.cs` (ADR 0010). Actuation lives in
`EngineSupervisor`.

The shell starts or attaches to a running engine, watches process exit, and applies
policy before launching again. The App 15s timer only calls `EnsureRunning` (attach
or launch if the mutex is gone); it does not kill a live engine.

## External watchers

`scripts/Watch-Kuro.ps1` is a thin logon helper: if `KuroShell.exe` is not running,
it starts Kuro via `Start-Kuro.ps1`. It does not force-kill or restart the engine
while the shell is alive.

Do not run multiple full supervisors that each try to restart the engine.

## Gate and CI

Repo gate scripts must not start, stop, or restart a live Kuro instance (phase 01).
