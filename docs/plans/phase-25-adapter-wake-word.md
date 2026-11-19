# Phase 25: Wake word adapter

- Size: M
- Depends on: phase 24
- Behavior change: yes, opt-in. Default `wake_word.enabled = false`.
- ADRs: 0008 (Trigger port), 0013 (model license)
- Protected paths: none for adapter folder; trigger registry only.

## Goal

Add a redistributable local wake-word adapter behind the Trigger port. Spike model
choice with license check; never enable by default. Cooldown, kill switch, no fire
during live turns.

## Scope

In: `kuro-triggers` or `engines/wake_word/`; adapter.toml; settings; FakeDetector
tests; models.lock entry after license verification; docs for enabling.

Out: Cloud wake word; training UI; Jarvis pack (26).

## Context card

Read:
- `docs/plans/05-jarvis-roadmap.md` item 5
- Trigger port from phase 12
- Open question 6
- `docs/adr/0013-license-and-third-party.md`

Create:
- Wake word adapter folder + README (license, model download)
- Unit tests with fake detector and FakeMic
- Hardware-tier test harness notes (manual)

Edit: triggers registry; example profile off; THIRD_PARTY_NOTICES / models.lock
when model chosen.

Do not touch: hotkey LL-hook; feature folders unrelated.

Commands:
```powershell
cargo test -p kuro-triggers
cargo xtask gate fast
```

## Tests first

- Disabled: detector never started.
- Enabled + fake accept: starts a turn once; cooldown blocks burst.
- Never fires during active turn.
- Missing model: doctor warns; engine runs without crash.

## Steps

1. Spike 1 to 2 candidate models; pick redistributable.
2. Implement adapter; wire Trigger.
3. Visible indicator via protocol state if available.
4. Document cancel word / PTT still works.

## Acceptance

- Default off; license documented.
- Tests cover false-accept mitigations at unit level.
- Doctor lists wake word status.

## Risks

- Non-commercial models: reject for template. If no good model, ship adapter
  interface + null engine and document.
