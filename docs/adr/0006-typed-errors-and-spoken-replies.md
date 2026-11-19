# ADR 0006: Typed errors, spoken replies

- Status: Accepted
- Date: 2026-11-14
- Related: mandate 5b; finding 19; constraint "speakable-string tool results"

## Context

Tools return strings. Callers decide success by parsing them:
`low.starts_with("success")` in `power()`, `"unable"` in `news_to_speech()`,
`result.starts_with("Success:")` in `meet.rs`. A device that never answered can
produce a spoken success (finding N14). The constraint to keep is that the user-facing
layer receives speakable strings.

## Options

1. Keep strings, add conventions.
2. Typed results inside, strings only at the edge.

## Decision

Option 2.

- `Feature::execute` returns `Result<Outcome, FeatureError>`.
- `Outcome { speech: Spoken, follow_up, data, undo }`. `Spoken` is built from a reply
  key in the manifest plus named arguments, so wording stays in data.
- `FeatureError` kinds: `NotConfigured`, `Unreachable`, `Timeout`, `Unauthorized`,
  `Rejected`, `InvalidInput`, `Busy`, `DeviceUnavailable`, `Disabled`, `Internal`.
  Each carries structured detail for logs, never for speech.
- The turn machine maps an error to speech: the feature's reply for that kind if the
  manifest defines one, else the default sentence for the kind from
  `kuro-features/data/errors.toml`.
- The API, the LLM tool-call path, and the audit log receive the typed result. The
  user hears and sees only the sentence.
- Success is claimed only when it was observed (for example a device reply).
- Timeouts and retries are declared per call through `ctx.http` with a mandatory
  timeout argument. Retries are allowed only for idempotent calls and at most once.

## Consequences

- No string prefix parsing anywhere; `speakable()` and its siblings are deleted.
- Health checks and metrics can count failures by kind.
- Each feature needs a small error mapping and tests for every kind it can produce.
- Raw upstream error bodies are never spoken.

## Enforced by

`clippy::unwrap_used` and `expect_used` in features; architecture rules 5 and 9; the
failure-injection naming check; the corpus (replies stay identical for existing
success paths).
