# Architecture decision records

One file per significant decision. Format: context, options, decision, consequences,
enforced by. Status is `Accepted` after the owner accepted the master plan; was `Proposed` until then. A superseded ADR stays in place and links to its replacement.

A change to a PROTECTED path (see `docs/plans/02-guardrails.md` section 3) must cite
an ADR with the commit trailer `ADR: NNNN`.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-portable-core-not-cross-platform.md) | Windows product with a portable core | Accepted |
| [0002](0002-crate-layering-and-size-budgets.md) | Crate layering and size budgets | Accepted |
| [0003](0003-feature-extension-model.md) | Feature extension model with explicit registration | Accepted |
| [0004](0004-data-driven-phrases-and-rituals.md) | Phrases, replies, and rituals as TOML data | Accepted |
| [0005](0005-config-profiles-secrets.md) | Config, profiles, and secrets | Accepted |
| [0006](0006-typed-errors-and-spoken-replies.md) | Typed errors, spoken replies | Accepted |
| [0007](0007-panic-strategy.md) | Panic strategy: unwind with two boundaries | Accepted |
| [0008](0008-turn-state-machine-and-ports.md) | Turn state machine and ports | Accepted |
| [0009](0009-protocol-contract-and-handshake.md) | Protocol contract and handshake | Accepted |
| [0010](0010-single-supervisor.md) | One supervisor | Accepted |
| [0011](0011-llm-provider-port.md) | LLM provider port | Accepted |
| [0012](0012-gate-tiers.md) | Gate tiers | Accepted |
| [0013](0013-license-and-third-party.md) | License and third-party terms | Accepted |
| [0014](0014-bench-tooling.md) | Bench tooling in xtask | Accepted |
| [0015](0015-phase-ordering.md) | Phase ordering | Accepted |
| [0016](0016-permission-tiers-and-confirmation.md) | Permission tiers and confirmation | Accepted |
| [0017](0017-logging-and-correlation-ids.md) | Logging and correlation ids | Accepted |
| [0018](0018-protected-paths.md) | Protected paths require an ADR | Accepted |
| [0019](0019-api-token-and-say-scope.md) | API token generation and `/say` scope | Accepted |

Template: [0000-template.md](0000-template.md).
