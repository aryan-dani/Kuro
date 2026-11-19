# Phase 24: Feature calendar

- Size: M
- Depends on: phase 23
- Behavior change: yes. Opt-in calendar read; default local ICS; Graph later.
- ADRs: 0016 (read_only)
- Protected paths: none.

## Goal

Ship `calendar` feature: next event, today, tomorrow, join_next. Provider: local
ICS file first; optional Graph adapter stubbed or behind flag. Open question 7
default.

## Scope

In: `features/calendar/`; ICS parser; FakeHttp fixtures for Graph path if present;
config keys; corpus cases; failure matrix tests.

Out: Wake word (25); morning brief ritual composition (26C can consume this).

## Context card

Read:
- `docs/plans/05-jarvis-roadmap.md` item 3
- Open question 7
- HttpClient port in FeatureContext

Create:
- `features/calendar/` with win/ics modules as needed
- Fixture ICS files under features/calendar/tests/fixtures
- Corpus phrases

Edit: registry; example profile `calendar.enabled = false` until user adds ICS path.

Do not touch: OAuth UI beyond documenting manual setup; shell unless card frame
display needed (use protocol card from 15).

Commands:
```powershell
cargo test -p kuro-features
cargo test -p kuro-router --test corpus
cargo xtask gate fast
```

## Tests first

- Fixture ICS: next/today speech golden.
- Missing file / empty day / all-day / timezone cases.
- Enabled false: feature not registered or rejects with spoken reason.

## Steps

1. ICS provider implementation.
2. Intents and spoken templates in feature.toml.
3. Optional Graph behind flag with FakeHttp tests only.
4. Card payload for agenda when shell supports it.

## Acceptance

- Read-only; no write path in v1.
- Default off in example profile.
- Failure matrix covered.

## Risks

- Timezone bugs: test with fixed FakeClock and explicit TZ in fixtures.
