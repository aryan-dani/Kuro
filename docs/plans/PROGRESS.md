# Execution progress (live)

Updated as agents land slices. Plan files in this folder stay the source of truth
for remaining work. Do not treat "first slice" as phase complete.

## Landed (working tree)

| Area | Status |
|------|--------|
| Plans 00-26 + README + ADRs Accepted | docs complete |
| Phase 00-08 platform spine | largely done |
| Phase 09-11 features + scaffolders | done |
| Phase 10 spoken bridge + live desk/music/meet/wa adapters | first slice |
| Phase 12-14 turn, engine split, router split | first slices |
| Phase 15 protocol + HTTP + pipe/WS HELLO | first slice |
| Phase 17-18 PageRegistry + RestartPolicy + thin Watch | first slices |
| Phase 20 listener Settings (loop still off) | first slice |
| Phase 21 barge-in + CancelSpeak + xtask bench | first slice |
| Phase 23-25 timers/calendar/weather/system/recall/wake Settings | first slices |
| Phase 26A Confirming + agent in VoiceTurn | first slice |
| Phase 26C morning_brief feature (router deferred) | first slice |

`cargo xtask gate fast` last known: green.

## Highest-value remaining

1. Hollow out / remove kuro-tools once all live ports sit behind features
2. Continuous listener when listener_enabled
3. Real wake word model + open-mic poll
4. Wire morning_brief phrases without clobbering workflows
5. Commit / PR dirty tree with ADR trailers on protected paths
  (0008/0016 turn+epoch, 0009 ipc/contract, 0010 supervisor)
