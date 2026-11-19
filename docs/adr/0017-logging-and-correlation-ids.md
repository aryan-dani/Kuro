# ADR 0017: Logging and correlation ids

- Status: Accepted
- Date: 2026-11-14
- Related: mandate 5b; findings 13, N9; phases 02, 12

## Context

Logging is `append_rotating_log` (free text, 1 MB, one backup) plus `eprintln!`,
which is discarded because the engine is built with `windows_subsystem = "windows"`.
There is no way to follow one voice turn through STT, routing, a feature, and TTS.
Timing lives in a separate CSV with wrong values for TTS and none for follow-ups.

## Options

1. Adopt `tracing` with a file layer. Standard, span-based, but spans do not follow
   work across plain threads without manual plumbing, and it adds dependencies and a
   subscriber to configure.
2. Extend the existing logger with structured fields and an explicit turn id.

## Decision

Option 2, in `kuro-core/src/log.rs`.

- One line per event in logfmt:
  `ts=... level=info turn=42 comp=stt ev=done ms=312 chars=18`.
- `turn_id` is a monotonically increasing integer per engine run, combined with a
  boot id in the file header. The turn machine creates it and passes it in `Ctx` and
  in every effect, so each line of a turn carries it. Proactive announcements and
  scheduled jobs get their own ids.
- API: `log.info(comp, ev, &[("ms", 312)])` on a handle that already holds the turn
  id. No global logger; the handle is injected.
- Levels: error, warn, info, debug. Default info; set by config.
- Secrets are never logged. Values from `SecretStore` have no `Display`.
- Transcripts and replies are logged at info because routing bugs cannot be diagnosed
  without them; `log.transcripts = false` turns that off. Logs never leave the
  machine. `kuro doctor --bundle` redacts them.
- Rotation stays (size-based, one backup), with the size in config.
- `eprintln!` and `println!` are forbidden outside tests and the CLI.
- `timing.csv` is produced from the same events, so the two cannot disagree.
- The shell shows the last error lines for a turn from `/status`.

`tracing` stays an option if async spans become necessary; the logfmt format maps to
it directly.

## Consequences

- `rg "turn=42" kuro.log` reconstructs a turn.
- No new dependency.
- Call sites must carry a handle, which the `Ctx` design already requires.

## Enforced by

A source scan for `eprintln!` and `println!` outside tests and `kuro-cli`;
`turn::tests::every_log_line_has_turn_id`; `log::tests::secret_is_not_printed`.
