# Phase 23: Feature timers, alarms, reminders

- Size: M
- Depends on: phase 22
- Behavior change: yes. New voice feature; default enabled with safe limits.
- ADRs: 0016 (reversible tier)
- Protected paths: none (feature folder only + registry lines).

## Goal

Ship `timers` / `reminders` feature using Scheduler, Announcer, Store, Clock.
No engine/router/turn edits beyond registry lines and routing cases in TOML.

## Scope

In: `features/timers/` (or timers + reminders); feature.toml intents; parser for
durations; persistence; failure modes from roadmap; routing corpus cases.

Out: Calendar (24); confirm policy (26A).

## Context card

Read:
- `docs/plans/05-jarvis-roadmap.md` item 1
- Feature scaffolder; Announcer/Scheduler APIs
- `docs/guides/add-a-feature.md`

Create:
- `kuro-rs/crates/kuro-features/src/features/timers/`
- Corpus additions under fixtures/phrases
- Unit tests with FakeClock / FakeStore

Edit: `features/mod.rs` two lines; example profile keys.

Do not touch: turn machine core; shell pages except optional later.

Commands:
```powershell
cargo xtask new-feature timers   # or hand-create matching layout
cargo test -p kuro-features
cargo test -p kuro-router --test corpus
cargo xtask gate fast
```

## Tests first

- FakeClock fires timer; announce called.
- Persist across restart; late delivery marked.
- Unparsable duration asks follow-up (Expect slot from phase 12).
- Max active limit.

## Steps

1. Scaffold feature; define slots and phrases.
2. Implement execute + on_event/schedule callbacks.
3. Add corpus cases.
4. Document in feature README.

## Acceptance

- All roadmap failure modes tested.
- Only feature folder + registry + corpus touched.
- Gate fast green.

## Risks

- Follow-up slot UX: reuse phase 12 Expect; do not invent parallel dialog system.
