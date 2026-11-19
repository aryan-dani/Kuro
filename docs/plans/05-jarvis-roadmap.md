# 05 Jarvis roadmap

Every feature below must fit the extension model in
[01-architecture.md](01-architecture.md): one folder, one registered line, no engine,
router, turn, IPC, or shell edit. Where the model was missing something, the gap is
listed in section 1 and closed once, in a named core phase, before the features that
need it.

## 1. Model gaps found, and where they are closed

| Gap | Needed by | Closed in |
|-----|-----------|-----------|
| A feature cannot ask a question and receive the answer | timers, reminders, WhatsApp, calendar | phase 12: `FollowUp::Expect(slot)` |
| Only the hotkey can start a turn | wake word | phase 12: `Trigger` port; `kuro-triggers` registry |
| Nothing can speak unprompted | timers, nudges, briefings | phase 22: `Announcer` with quiet hours and rate limits |
| No persisted jobs | timers, reminders, briefings | phase 22: `Scheduler` |
| No store | memory, timers, audit log | phase 22: `Store` (SQLite, already a workspace dependency) |
| No internal events | presence automation, hub events, nudges | phase 22: `EventBus`, `Feature::on_event` |
| No record of what was said and done | "repeat that", "do that again", "undo that" | phase 20: turn log; `Outcome.undo` |
| No confirmation step | sends, irreversible actions, LLM tool calls | phase 26 part A: `Confirming` state driven by `tier` |
| LLM cannot call features | agentic use | phase 26 part A: tool calling over manifest `llm_description` and slots |
| Features known only at compile time | MCP, n8n workflows, hub tools | phase 09: `DynamicSource` |
| A feature cannot show anything | sources, agenda, QR code | phase 15: generic `card` frame (title, lines, links, image path) |

After these, no feature in this file needs a core edit. Two honest exceptions are
marked "core" below because they are capabilities, not features.

## 2. Permission tiers (ADR 0016)

| Tier | Examples | Voice command | LLM tool call |
|------|----------|---------------|---------------|
| `read_only` | time, agenda, weather | runs | runs |
| `reversible` | desk power, volume, timers | runs | runs, logged |
| `external_send` | WhatsApp, email, calendar write | runs when the user named recipient and content; else confirm | always confirm |
| `irreversible` | run script, delete, shutdown | always confirm | always confirm |

Every executed action is written to the audit log with turn id, source (voice, LLM,
ritual, schedule), tier, and result.

## 3. Ranked backlog

Fields: value; effort (S, M, L) and risk; adapters; tier; config keys; manifest
(intents); failure modes; tests. "v1" marks the shortlist.

### 1. Timers, alarms, reminders (v1, phase 23)
- Value: "set a timer for ten minutes", "remind me at five to call Sam", "cancel the timer".
- Effort S, risk low. Adapters: Scheduler, Announcer, Store, Clock. Tier: reversible.
- Config: `timers.enabled`, `timers.max_active`, `timers.chime`, `timers.repeat_announce_s`.
- Manifest: `timers.set(duration, label)`, `timers.cancel(label)`, `timers.list`, `reminders.set(when, text)`, `reminders.list`.
- Failures: unparsable duration (ask), limit reached, fire during a live turn (queue), fire while the engine was off (announce late, marked late), quiet hours (chime only).
- Tests: FakeClock fires; persistence across restart with FakeStore; late delivery; parser property tests.

### 2. Conversation memory: repeat, again, undo (v1, core part in phase 20)
- Value: "repeat that", "say it shorter", "do that again", "undo that", "what did I just ask".
- Effort M, risk low. Adapters: Store (turn log). Tier: read_only; undo inherits the original tier.
- Config: `recall.enabled`, `recall.keep_turns`.
- Manifest: `recall.repeat`, `recall.shorter`, `recall.again`, `recall.undo`, `recall.last_question`.
- Failures: nothing to repeat, action not undoable (say so), undo target expired.
- Tests: scenario tests over the turn log; undo of desk power with a fake device.

### 3. Calendar: next event and agenda (v1, phase 24)
- Value: "what's next", "what's on today", "join my next meeting".
- Effort M, risk medium (OAuth consent). Adapters: HttpClient, shared Google auth, Clock. Tier: read_only.
- Config: `calendar.enabled`, `calendar.calendars`, `calendar.lookahead_hours`, `calendar.speak_max`.
- Manifest: `calendar.next`, `calendar.today`, `calendar.tomorrow`, `calendar.join_next`.
- Failures: not connected, consent missing for the scope, token revoked, timeout, empty day, all-day events, time zones.
- Tests: fixture JSON through FakeHttp; golden speech; failure matrix.

### 4. Computer control basics (v1, phase 26 part B)
- Value: "volume thirty", "mute", "next track", "lock the PC", "open the editor".
- Effort S, risk low. Adapters: a `SystemControl` trait inside the feature (`win.rs`). Tier: reversible.
- Config: `system.enabled`, `system.apps` (name to path, in the profile), `system.volume_step`.
- Manifest: `system.volume_set(level)`, `system.volume_step(dir)`, `system.mute`, `system.media(key)`, `system.lock`, `system.open_app(name)`.
- Failures: unknown app, missing path, no audio endpoint.
- Tests: fake `SystemControl` records calls; level parsing; routing cases.

### 5. Wake word (v1, opt-in, phase 25)
- Value: hands-free activation next to push-to-talk.
- Effort M, risk medium (false accepts, CPU, model license). Adapters: Trigger, Mic frames, ONNX runtime (existing). Tier: not applicable.
- Config: `wake_word.enabled` (false), `wake_word.model`, `wake_word.threshold`, `wake_word.cooldown_ms`, `wake_word.only_on_ac`.
- Manifest: adapter manifest `adapter.toml` (settings and model entry).
- Failures: model missing, mic lost, false accept (cooldown, visible indicator, cancel word), high CPU (duty cycle).
- Tests: fake detector and FakeMic; never fires during a live turn; kill-switch; hardware-tier false-accept run on silence and music clips.

### 6. Confirmation policy and LLM tool calling (v1, core, phase 26 part A)
- Value: "turn the desk on and play something", chained requests in natural language.
- Effort M, risk medium (wrong action). Adapters: Llm with tool calls. Tier: enforced by table in section 2.
- Config: `agent.enabled`, `agent.max_steps` (3), `agent.confirm` (by tier), `agent.allow_tiers`.
- Manifest: uses each feature's `llm_description` and `slots`.
- Failures: model emits an unknown tool or bad arguments (reject, say so), loop (step cap), cancel during plan, confirmation timeout (treat as no).
- Tests: FakeLlm scripted tool calls; tier matrix; step cap; audit log rows.

### 7. Morning brief with weather (v1, phase 26 part C)
- Value: one phrase or a scheduled time gives calendar, weather, inbox, headlines.
- Effort S after 1 and 3. Risk low. Adapters: rituals engine, HttpClient (weather). Tier: read_only.
- Config: `weather.enabled`, `weather.latitude`, `weather.longitude`, `weather.units`; the brief is a ritual file.
- Manifest: `weather.now`, `weather.today`; ritual `morning_brief` with four steps.
- Failures: any step failing must not stop the others; location unset.
- Tests: ritual with one failing step; weather fixture JSON.

### 8. "What can I say", feature toggles, health page (v1, phase 26 part D)
- Value: discoverability and trust; every feature visible with its state.
- Effort S. Risk low. Adapters: none (reads manifests and `/status`). Tier: read_only.
- Config: none beyond each feature's `enabled`.
- Manifest: `help.what_can_i_say`, `help.feature(name)`.
- Failures: none beyond rendering.
- Tests: shell view-model tests on fixture `/status` JSON; help text generated from two fake manifests.

### 9. Long-term memory
- Value: "remember that the router password is on the fridge", "what do you know about my sister".
- Effort M, risk medium (privacy). Adapters: Store, router embedder for retrieval. Tier: reversible.
- Config: `memory.enabled`, `memory.max_items`, `memory.use_in_llm`.
- Manifest: `memory.remember(text)`, `memory.recall(query)`, `memory.forget(query)`, `memory.list`.
- Failures: nothing found, store locked, forget ambiguity (ask).
- Tests: FakeStore; retrieval ranking with the hash embedder; forget requires confirmation.

### 10. Proactive nudges and quiet hours
- Value: "meeting in five minutes", urgent mail, low battery, long build finished.
- Effort S each after phase 22. Risk medium (annoyance). Adapters: EventBus, Announcer. Tier: read_only.
- Config: `quiet_hours.start`, `quiet_hours.end`, `nudges.max_per_hour`, one switch per nudge.
- Manifest: `dnd.on`, `dnd.off`, `dnd.status`; event subscriptions per nudge.
- Failures: nudge during a turn (queue), during quiet hours (card only), bursts (rate limit).
- Tests: FakeClock and bus events; rate limit; quiet hours.

### 11. Clipboard actions
- Value: "summarize my clipboard", "translate this to Spanish", "rewrite this politely".
- Effort S, risk low. Adapters: Clipboard, Llm. Tier: reversible (writes the clipboard on request).
- Config: `clipboard.enabled`, `clipboard.max_chars`.
- Manifest: `clipboard.summarize`, `clipboard.translate(lang)`, `clipboard.rewrite(style)`.
- Failures: empty clipboard, non-text content, too long, LLM off.
- Tests: fake clipboard and FakeLlm.

### 12. Focus mode
- Value: "focus for 25 minutes": timer, do-not-disturb, optional desk light.
- Effort S after 1 and 10. Risk low. Adapters: rituals. Tier: reversible.
- Config: ritual file. Manifest: ritual `focus(duration)`.
- Failures: a step failing does not block the timer. Tests: ritual scenario.

### 13. Quick capture
- Value: "note: buy filters", "add a task to call the bank".
- Effort S, risk low. Adapters: file append, or a workflow. Tier: reversible.
- Config: `capture.enabled`, `capture.notes_file`, `capture.task_workflow`.
- Manifest: `capture.note(text)`, `capture.task(text)`.
- Failures: file locked, workflow down. Tests: temp file; FakeHttp.

### 14. More devices: Home Assistant adapter and data-driven scenes
- Value: any light or plug, not only one Tuya plug.
- Effort M, risk medium. Adapters: Device port, HttpClient. Tier: reversible.
- Config: `home_assistant.url`, secret token, `devices` map in the profile.
- Manifest: `devices.turn(name, state)`, `devices.scene(name)`.
- Failures: unreachable, unknown entity, unauthorized. Tests: fixture JSON, failure matrix.

### 15. Presence automations
- Value: rituals triggered by arriving or leaving, not only by voice.
- Effort S after phases 19 and 22. Risk medium (surprise actions). Tier: by ritual steps.
- Config: ritual files gain `on_event = "presence.home"`. Tests: bus event runs the ritual once per cooldown.

### 16. Earcons, whisper mode, "say it shorter"
- Value: faster, quieter feedback. Effort S. Risk low. Adapters: AudioOut.
- Config: `earcons.enabled`, `voice.whisper_hours`. Tests: effect order in turn scenarios.

### 17. Calculator, units, translation
- Value: instant local answers. Effort S. Risk low. Tier: read_only.
- Manifest: `calc.eval(expr)`, `units.convert(value, from, to)`, `translate.say(text, lang)`.
- Failures: unparsable expression. Tests: table-driven and property tests.

### 18. Web answers with spoken citations
- Value: current facts. Effort M. Risk medium (cloud, cost, prompt injection). Adapters: HttpClient to a search API, Llm. Tier: read_only.
- Config: `web.enabled` (false), `web.provider`, secret key, `privacy.allow_cloud`.
- Failures: offline, quota, no result. Tests: fixtures; results treated as data, never as instructions.

### 19. Dictation into the focused app
- Value: voice typing. Effort M. Risk medium (types into the wrong window). Tier: external_send.
- Config: `dictation.enabled`, `dictation.stop_word`. Failures: focus change mid-dictation (stop).
- Tests: fake keyboard sink; focus change.

### 20. Music ducking while listening
- Value: Kuro hears you over music. Effort M. Risk low. Adapters: audio session volume (`win.rs`).
- Config: `ducking.enabled`, `ducking.level`. Failures: restore must always run (RAII). Tests: fake session.

### 21. Project helpers: open project, git status, run saved script
- Value: "open the Kuro project", "git status", "run the backup script".
- Effort S to M. Risk: scripts are `irreversible` and always confirmed.
- Config: `projects` and `scripts` maps in the profile. Tests: fake process runner; confirmation required.

### 22. MCP client
- Value: reuse existing MCP servers as Kuro tools.
- Effort M to L. Risk high (untrusted tools). Adapters: DynamicSource, supervised worker. Tier: `external_send` by default per tool.
- Config: `mcp.servers` list. Failures: server crash, slow tool, schema drift. Tests: an in-process fake server.

### 23. Local RAG over notes
- Value: "what did I write about the router setup". Effort L. Risk medium (index size, privacy).
- Adapters: Store, embedder. Config: `rag.folders`, `rag.max_mb`. Tests: tiny corpus with the hash embedder.

### 24. Shell items
- First-run wizard (M, phase 21): mic test, model download with checksums, profile creation.
- Transcript history with search (S). Notification list for proactive items (S, after phase 22).
- Audit log viewer (S, after phase 26 part A). Calendar write and email send (M, `external_send`).

Cut for now, with reasons: speaker verification (large effort, weak security value on
a personal desk), theme editor (a theme file is enough), brightness control (hardware
specific and unverified), general window management (low value per effort).

## 4. v1 shortlist

1. Timers, alarms, reminders (phase 23)
2. Conversation memory: repeat, again, undo (phase 20 core, feature in phase 26)
3. Calendar read (phase 24)
4. Computer control basics (phase 26 part B)
5. Wake word, opt-in (phase 25)
6. Confirmation policy and LLM tool calling (phase 26 part A)
7. Morning brief with weather (phase 26 part C)
8. "What can I say", toggles, health page (phase 26 part D)

Barge-in and streaming LLM to TTS give as large a jump as any of these, but they are
product quality work on the turn machine and live in phases 20 and 21.

## 5. Worked examples

Each phase file lists the exact files created and the registered line, and states
which core files are untouched.

| Example | Phase file | Files created | Registered | Engine, router, turn edits |
|---------|------------|---------------|------------|----------------------------|
| Timers and reminders | [phase-23](phase-23-feature-timers-reminders.md) | 9 files in `features/timers/` | 2 lines in `features/mod.rs` | none |
| Calendar | [phase-24](phase-24-feature-calendar.md) | 10 files in `features/calendar/` | 2 lines in `features/mod.rs` | none |
| Wake word | [phase-25](phase-25-adapter-wake-word.md) | 6 files in `kuro-triggers/src/wake_word/`, 1 entry in `models.lock.toml` | 1 line in `kuro-triggers/src/registry.rs` | none |
