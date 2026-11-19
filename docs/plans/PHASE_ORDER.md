# Phase order

| Phase | File | Size | Depends |
|-------|------|------|---------|
| 00 | [characterization](phase-00-characterization-tests.md) | M | none |
| 01 | [minimal guardrails](phase-01-minimal-guardrails.md) | M | 00 |
| 02 | [crash containment](phase-02-crash-containment.md) | M | 01 |
| 03 | [security fixes](phase-03-security-fixes.md) | M | 01 |
| 04 | [music playback](phase-04-music-playback-fix.md) | S | 01 |
| 05 | [repo hygiene](phase-05-repo-hygiene.md) | M | 02, 03, 04 |
| 06 | [AGENTS and cursor rules](phase-06-agents-and-rules.md) | S | 05 |
| 07 | [typed config](phase-07-typed-config.md) | M | 06 |
| 08 | [init and doctor](phase-08-init-and-doctor.md) | M | 07 |
| 09 | [extension model](phase-09-extension-model.md) | L | 08 |
| 10 | [migrate features](phase-10-migrate-features.md) | L | 09 |
| 11 | [scaffolders](phase-11-scaffolders.md) | M | 10 |
| 12 | [turn state machine](phase-12-turn-state-machine.md) | L | 11 |
| 13 | [split engine main](phase-13-split-engine-main.md) | M | 12 |
| 14 | [split router and TTS](phase-14-split-router-tts.md) | M | 13 |
| 15 | [protocol contract](phase-15-protocol-contract.md) | M | 12 |
| 16 | [HUD and capsule split](phase-16-hud-capsule-split.md) | M | 15 |
| 17 | [MainWindow pages](phase-17-mainwindow-pages.md) | M | 16 |
| 18 | [single supervisor](phase-18-single-supervisor.md) | M | 17 |
| 19 | [wiring and parity](phase-19-wiring-and-parity.md) | M | 14, 18 |
| 20 | [listener and memory](phase-20-listener-and-memory.md) | L | 19 |
| 21 | [barge-in and performance](phase-21-barge-in-and-perf.md) | L | 20 |
| 22 | [template polish](phase-22-template-polish.md) | M | 21 |
| 23 | [timers reminders](phase-23-feature-timers-reminders.md) | M | 22 |
| 24 | [calendar](phase-24-feature-calendar.md) | M | 23 |
| 25 | [wake word adapter](phase-25-adapter-wake-word.md) | M | 24 |
| 26 | [Jarvis pack](phase-26-jarvis-pack.md) | L | 25 |

Phases 05 and 06 can run in parallel with leftover work from 02 to 04. Phases 15
to 17 can run in parallel with 12 to 14 once protocol fixtures from 00 exist.

## v1 Jarvis shortlist

See [05-jarvis-roadmap.md](05-jarvis-roadmap.md): timers, memory, calendar,
computer control, wake word, confirm + LLM tools, morning brief, discoverability.
