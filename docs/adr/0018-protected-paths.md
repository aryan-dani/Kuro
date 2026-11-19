# ADR 0018: Protected paths require an ADR

- Status: Accepted
- Date: 2026-11-14
- Related: mandate 5d; `docs/plans/02-guardrails.md` section 3; phase 01

## Context

A few areas break the whole product or its security when changed carelessly: the
keyboard hook, the engine-shell contract, turn epoch and cancellation, secret and
token handling, the supervisor, capsule positioning, and the guardrails themselves.
Agents will make most changes in this repository, and prose warnings are easy to
miss.

## Options

1. CODEOWNERS only. Works on GitHub with branch protection, not locally, and not for
   a solo owner who merges their own work.
2. A local and CI check that blocks changes to listed paths unless the change cites a
   decision record.
3. Both.

## Decision

Option 3.

- `kuro-rs/xtask/data/protected_paths.txt` is the single list. `.github/CODEOWNERS`
  is generated from it.
- `cargo xtask check protected` fails when a changed file matches the list and a
  commit touching it lacks the trailer `ADR: NNNN`, or when `docs/adr/NNNN-*.md` does
  not exist. It runs on the staged diff in the fast gate and on the PR range in CI.
- An ADR for a protected change states what invariant is affected and which test
  proves it still holds. Small fixes may cite an existing ADR that already covers the
  change (for example the security fixes cite ADR 0019).
- The list itself and the checks are protected.
- A Cursor rule (`protected.mdc`) tells agents to stop and ask before editing these
  paths.

Initial list and later additions are in `docs/plans/02-guardrails.md` section 3.

## Consequences

- Touching the hook or the contract takes a deliberate extra step and leaves a
  record.
- A determined author can still write a trailer; the goal is to stop accidents and
  force a pause, not to stop malice.
- Phases in the master plan that edit protected paths name the ADR to cite, so
  executing agents are not blocked.

## Enforced by

`cargo xtask check protected` (fast gate and the `protected` CI job); unit tests of
the check with a temporary git repository.
