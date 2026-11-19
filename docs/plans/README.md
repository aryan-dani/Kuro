# Kuro master plan

Index for the template / modular / Jarvis rewrite. Verified against `main` @
`f661122`. Code wins when it disagrees; see [00-findings.md](00-findings.md).

**Start:** [phase-00-characterization-tests.md](phase-00-characterization-tests.md).
**Live status:** [PROGRESS.md](PROGRESS.md).

Priorities: (1) template, (2) robustness, (3) agent modularity, (4) product,
(5) Jarvis. Items 1 to 3 must not change user-visible behavior except deliberate
defect fixes with tests.

## Decisions

ADRs [0001](../adr/0001-portable-core-not-cross-platform.md) to
[0019](../adr/0019-crypto-token-and-say-scope.md). Top ten listed in
[01-architecture.md](01-architecture.md).

## Pack map

| File | Purpose |
|------|---------|
| [00-findings.md](00-findings.md) | Verified findings |
| [00-failure-modes.md](00-failure-modes.md) | Failure tables |
| [01-architecture.md](01-architecture.md) | Target tree and turn machine |
| [01-split-plans.md](01-split-plans.md) | Oversize file splits |
| [02-guardrails.md](02-guardrails.md) | Gate, CODEOWNERS, agent-ability |
| [03-tests-and-ci.md](03-tests-and-ci.md) | Corpus and CI |
| [04-repo-hygiene.md](04-repo-hygiene.md) | License and personal data |
| [05-jarvis-roadmap.md](05-jarvis-roadmap.md) | Jarvis shortlist |
| [06-risks.md](06-risks.md) | Risks |
| [07-first-agent-tasks.md](07-first-agent-tasks.md) | Starter tasks |
| [PHASE_ORDER.md](PHASE_ORDER.md) | Phase 00 to 26 table |
| `phase-NN-*.md` | One mergeable phase each |

## How to execute

Open one phase file and its context card. Tests first when stated. Leave
`cargo xtask gate fast` green. One phase per PR. ADR trailers on protected paths.
