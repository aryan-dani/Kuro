# Phase 19: Wiring and parity

- Size: M
- Depends on: phases 14 and 18
- Behavior change: hub and other DynamicSources may become optionally wired (off
  by default). Parity gaps closed with tests. No forced enable of remote hub.
- ADRs: 0003 (DynamicSource), open question 4 default
- Protected paths: none typically.

## Goal

Close "unwired but compiled" gaps: hub DynamicSource behind config flag; inbox /
timing env keys in allowlist via settings; end-to-end parity tests for voice path
vs `/say` within ADR 0019 scope; document feature flags in example profile.

## Scope

In: Wire hub when `hub.enabled`; VoiceTurn passes real DynamicSource when
configured; parity tests; fix remaining allowlist/settings holes from findings;
n8n example sanitize if needed.

Out: Listener/memory product (20); barge-in (21).

## Context card

Read:
- Findings: hub unwired (`VoiceTurn::run` passes None)
- `docs/plans/00-failure-modes.md` hub/inbox
- Feature registry DynamicSource
- ADR 0019 `/say` scope

Create:
- Tests for hub off by default; hub on with fake source
- Parity table tests where missing

Edit: engine wiring; example profile; feature flags.

Do not touch: Jarvis new features; wake word.

Commands:
```powershell
cargo test -p kuro-engine
cargo test -p kuro-features
cargo xtask gate fast
```

## Tests first

- Default profile: hub path not contacted (fake would fail if called).
- Enabled hub: event reaches feature on_event or equivalent.
- `/say` respects phrase + read-only rules from phase 03/0019.

## Steps

1. Inventory unwired ports from findings.
2. Wire behind settings defaults off.
3. Add parity tests.
4. Update failure-mode doc checkboxes.

## Acceptance

- No silent None for hub when enabled.
- Example profile safe for template.
- Gate green.

## Risks

- Re-enabling hub with bad defaults: keep off; document.
