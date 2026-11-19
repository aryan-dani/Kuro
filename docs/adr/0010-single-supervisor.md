# ADR 0010: One supervisor

- Status: Accepted
- Date: 2026-11-14
- Related: mandate 5b; findings 15, N12, N13, N26; phase 18

## Context

Three paths can restart the engine: the `Exited` handler in `EngineSupervisor`
(with `Thread.Sleep` inside the handler), a 15 second watchdog in `App`, and
`scripts/Watch-Kuro.ps1` running under its own mutex. They exist because each covers
a gap: process exit, a hung engine (stale heartbeat), and "nothing is running after
logon". They can race. The shell has no single-instance guard, `/shutdown` does not
exit the engine, and liveness is judged by a process name containing "kuro".

## Options

1. Keep all three and add coordination flags (the `BUILDING` flag is an example).
2. Make the PowerShell watcher the only supervisor. Works headless, but polling
   scripts are slow, invisible, and hard to test.
3. A strict tree: the operating system starts the shell, the shell supervises the
   engine, the engine supervises its threads.

## Decision

Option 3.

- Task Scheduler logon task starts the shell (or the engine in headless mode) with
  restart on failure. This behavior is unverified for a WinUI application and is the
  first thing phase 18 tests.
- `KuroShell` holds a single-instance mutex and contains the only engine restart
  authority. The policy is a pure class in `KuroShell.Core/Supervisor` with tests:
  backoff 1, 2, 5, 15, 60 seconds, reset after 5 healthy minutes, stop after 5
  restarts in 10 minutes and show a tray action.
- Hung engine detection moves into two places: the supervisor reads heartbeat age
  from `/status`, and the engine runs a watchdog thread that exits with code 70 when
  the turn thread stalls.
- Engine exit codes carry intent: 0 no restart, 64 invalid config (no restart, show
  the message), 70 restart.
- `/shutdown` exits the process with code 0.
- Liveness uses the singleton mutex and the pid in `kuro.lock`, not a name match.
- `Watch-Kuro.ps1`, `Watch-KuroWifi.ps1`, and the `App` watchdog are deleted.
- Binaries run from `%LOCALAPPDATA%\Kuro\bin`, so builds never fight a running copy.

## Consequences

- One place to reason about restarts, with unit tests.
- If the shell is killed, the engine keeps serving push-to-talk; the logon task or
  the user brings the shell back and it attaches.
- Headless mode has no hang detection beyond the engine's own watchdog.

## Enforced by

`SupervisorTests` (backoff, give up, stale heartbeat, second instance); the manual
kill checklist in phase 18; PROTECTED status of the supervisor files.
