# Phase 22: Template polish and platform gaps

- Size: M
- Depends on: phase 21
- Behavior change: mostly infrastructure (Scheduler, Announcer, Store, EventBus)
  used by later Jarvis phases. Quiet hours and rate limits default safe.
- ADRs: from roadmap model gaps; 0013 theme default
- Protected paths: none typically.

## Goal

Close remaining extension-model gaps before Jarvis features: Store, Scheduler,
Announcer (quiet hours), EventBus + Feature::on_event; theme file
`themes/kuro.toml`; README 10-minute path verified; release guide; brand
invariants become theme-valid checks.

## Scope

In: Core ports and default adapters; example ritual using Announcer; theme move;
polish README/CONTRIBUTING; cargo-deny if not present; models.lock filled.

Out: Timers feature UI phrases (23); calendar (24).

## Context card

Read:
- `docs/plans/05-jarvis-roadmap.md` section 1 gaps
- `docs/plans/04-repo-hygiene.md` remaining items
- Open questions 1 and 8 defaults

Create:
- Store / Scheduler / Announcer / EventBus modules (crates as decided in ADR/arch)
- `themes/kuro.toml`
- Example ritual file under profiles/example

Edit: feature context to expose new ports; invariants script theme check;
README timing note.

Do not touch: adding full Jarvis phrase packs beyond examples.

Commands:
```powershell
cargo test -p kuro-core
cargo test -p kuro-features
cargo xtask gate full
```

## Tests first

- Scheduler with FakeClock fires once; persists across reopen with FakeStore.
- Announcer respects quiet hours (chime vs speak policy).
- EventBus delivers to registered feature.

## Steps

1. Implement ports with fakes in testkit.
2. Wire into FeatureContext.
3. Theme file + invariant update.
4. Verify cold-start docs honesty (prebuilt binary note).

## Acceptance

- Roadmap gaps for Store/Scheduler/Announcer/EventBus closed.
- Template README accurate; gate full green.
- Example profile still harmless.

## Risks

- Scope creep into features: stop at ports + one example ritual.
