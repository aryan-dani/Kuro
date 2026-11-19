# ADR 0015: Phase ordering

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 10; `docs/plans/README.md`

## Context

The owner's order: characterization tests, then only crashers and security fixes,
then restructure, then robustness per feature as each is migrated. Items 1 to 3 of
the priorities (template, robustness, agent-friendly modularity) must not change
user-visible behavior except for deliberate defect fixes with regression tests.

## Options

1. The owner's order as written.
2. Restructure first, fix on the clean structure. Faster to a tidy tree, but every
   move risks silent behavior change with no net.
3. The owner's order with four adjustments.

## Decision

Option 3. The principle stands; the adjustments are:

1. Minimal guardrails land immediately after the characterization tests (phase 01),
   before any fix. Every later phase is executed by an agent, so the size ratchet,
   text checks, fast gate, and `AGENTS.md` pay off from the second PR on.
2. Config and profiles (phases 07, 08) come before the extension model. Features
   need `Ctx.config`, and personal data needs somewhere to go before it can leave
   the code.
3. Feature migration (phases 09 to 11) comes before the turn machine (phase 12).
   Moving the turn loop is the riskiest refactor; it is safer once feature execution
   has one typed interface.
4. Wiring fixes that are neither crashers nor security issues (hub, Wi-Fi policy,
   timing, the Ollama message) wait until the seam they need exists (phase 19),
   because each then becomes a small change with a headless test. The exception is
   music, which gets its own early phase (04) because it is self-contained and the
   owner listed it as critical.

Robustness is not a separate sweep. The checklist (typed errors, timeout, retry
rule, health, logging with turn id, failure-injection tests, kill-switch) is the
definition of done for each feature migration.

Order: safety net (00, 01), critical fixes (02 to 04), hygiene and template (05,
06), config and profiles (07, 08), extension model (09 to 11), engine (12 to 14),
protocol and shell (15 to 18), wiring (19), product quality (20, 21), Jarvis (22 to
26).

## Consequences

- No user-visible gain from the restructure itself until phase 08, apart from the
  early fixes. This is the price of safety.
- Phases 05 and 06 can run in parallel with 02 to 04. Phases 15 to 17 can run in
  parallel with 12 to 14.
- A feature is touched once, not twice.

## Enforced by

Dependencies declared in each phase file; the corpus-unchanged rule for refactor
PRs; the migration counter in `docs/plans/README.md`.
