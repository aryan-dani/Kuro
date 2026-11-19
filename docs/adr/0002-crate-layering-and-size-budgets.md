# ADR 0002: Crate layering and size budgets

- Status: Accepted
- Date: 2026-11-14
- Related: mandate 5a; finding N23; `docs/plans/01-architecture.md` section 2

## Context

Three dependency edges are inverted today: `kuro-core` depends on `ort` and
`windows-sys`; `kuro-router` depends on `kuro-tools` and executes tools; `kuro-ipc`
depends on the router, tools, and TTS. Eleven files exceed 400 lines. An agent must
read 700-line files to change one phrase.

## Options

1. Keep the ten crates, add rules by convention only.
2. One crate per feature. Clean isolation, but slow builds, heavy boilerplate, and a
   workspace file every feature must edit.
3. A fixed set of layer crates with a mechanical dependency table, features as
   folders inside one crate, and a size ratchet.

## Decision

Option 3. The dependency table in `docs/plans/01-architecture.md` section 2 is the
law. `kuro-core` holds pure domain code, port traits, and the feature API. Adapters
depend only on `kuro-core` and `kuro-config`. `kuro-engine` is the only composition
root. Features are folders in `kuro-features`.

Size budgets: target under 200 lines per hand-written source file, hard cap 400,
functions under about 60 lines (`clippy::too_many_lines`). Generated and pure data
files are exempt when their first line is `// @generated`. Legacy offenders sit in an
allowlist that can only shrink.

The public surface of every crate is its `lib.rs` re-export list plus a doc comment;
other crates may not import from internal modules (modules are private by default).

## Consequences

- New crates: `kuro-config`, `kuro-protocol`, `kuro-features`, `kuro-llm`,
  `kuro-turn`, `kuro-triggers`, `kuro-cli`, `xtask`. `kuro-tools` is removed.
- More files, each small. Navigation relies on the generated repo map.
- A few splits cost real refactoring (`wa_neonize.rs`, `main.rs`); most are moves.

## Enforced by

`cargo xtask check arch` (rules 1, 2, 11, 15); `cargo xtask check size`.
