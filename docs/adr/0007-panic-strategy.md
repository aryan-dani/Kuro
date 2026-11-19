# ADR 0007: Panic strategy: unwind with two boundaries

- Status: Accepted
- Date: 2026-11-14
- Related: mandate 5b; findings 2, 20, N5; phase 02

## Context

The release profile sets `panic = "abort"`. `ToolRegistry::execute` and `try_handle`
wrap `catch_unwind`, which does nothing under abort, so "tools never panic the
engine" is false. `.expect("bind")` on a busy port kills the whole engine, including
push-to-talk. The engine owns the global hotkey, so a crash is the worst user-visible
failure.

## Options

1. Keep abort, remove the `catch_unwind` calls, rely on the supervisor. Honest and
   small, but any bug in any feature costs a full restart (model reload, seconds
   without a hotkey).
2. Keep abort, run risky tools in child processes. Strong isolation, heavy: IPC,
   packaging, latency, and the WhatsApp DLL still needs a host.
3. Switch to unwind, catch at a small number of boundaries, keep the supervisor as
   the last line.

## Decision

Option 3.

- `panic = "unwind"` in the release profile.
- Exactly two catch boundaries: feature execution, and effect execution on the turn
  thread. A caught panic becomes `FeatureError::Internal`, is logged with `turn_id`
  and a backtrace, and the turn ends in Idle.
- A panic hook writes to `kuro.log` before unwinding starts.
- Panics are never control flow. `unwrap` and `expect` are denied on external input.
- Every busy flag or cooldown is an RAII guard, so unwinding releases it.
- Supervised threads that die are restarted by the in-process thread supervisor;
  failure of the hotkey thread or repeated failure of the turn thread exits with
  code 70 so the shell supervisor restarts the process.
- FFI callbacks (the neonize bridge) catch panics at the boundary and never unwind
  into foreign code.
- Port bind failure is handled as an error with retry, not a panic.

Process isolation is kept as a later option for third-party code such as MCP servers,
which are separate processes by design.

## Consequences

- Slightly larger binary; no measurable hot-path cost.
- State that a panicking feature left behind must be safe to reuse: features hold no
  shared mutable state, and mutex poisoning is handled explicitly.
- The supervisor is still required for aborts in native code (whisper.cpp, ONNX
  Runtime, the Go DLL).

## Enforced by

`engine::tests::panic_in_feature_keeps_ptt`; `meet::tests::busy_released_on_panic`;
architecture rule 7; a check that `catch_unwind` appears only in the two boundary
files and the FFI callback file.
