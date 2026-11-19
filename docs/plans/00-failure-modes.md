# 00 Failure-mode tables

Companion to [00-findings.md](00-findings.md). One table per integration.

Columns:
- Failure: what can go wrong.
- Today: how it is detected and what the user gets now (from code reading; not
  runtime-tested).
- Target: detection, what the user hears, what the HUD shows.
- Recovery: what happens next without user action.
- Test: the regression or failure-injection test that proves the target, and the phase
  that adds it. Test names are the names the phase must use.

Common target rules (ADR 0006, ADR 0017):
- Every failure maps to a typed `FeatureError` kind and a reply key in the feature's
  `feature.toml`. The spoken sentence is data, the kind is code.
- The HUD shows `error` for 1.5 s and always returns to idle.
- Every failure writes one log line with `turn_id`, `feature`, `kind`, `ms`.
- Health for each integration appears in `/status` and `kuro doctor`.

## 1. Smart plug (Tuya local)

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Not configured (no device id or key) | Tries `127.0.0.1:6668`, fails, error string parsed by keyword | `NotConfigured`; "The desk plug is not set up yet." | None; doctor explains the missing keys | `desk_power::tests::not_configured_is_spoken` (10) |
| Device unreachable | 2 s connect timeout, error string | `Unreachable`; "The desk plug is not responding." | One retry, then re-resolve the cached IP on next call | `desk_power::tests::unreachable_retries_once` (10) |
| Wrong key or protocol version | Write succeeds, success is spoken, nothing happens | Read the device reply within 2 s; `Rejected`; "The desk plug refused the command." | None; health goes `down` | `desk_power::tests::no_ack_is_not_success` (10) |
| Reused nonce or sequence | Fixed nonce and seq 1 on every frame | Random GCM nonce, increasing seq | Not applicable | `tuya::tests::nonce_never_repeats` (03) |
| Two commands at once | No lock; frames interleave | Per-device mutex with 3 s wait; `Busy` | Second caller waits, then runs | `desk_power::tests::concurrent_calls_serialize` (10) |

## 2. Music

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Stream dropped after start | Playback ends or is silent; reply already spoken | Player thread owns the output stream | Not applicable | `music::tests::stream_outlives_play_call` (04) |
| Pause, resume, stop do nothing | Flags flip only | Commands reach the sink; stop is idempotent | Not applicable | `music::tests::pause_resume_stop_reach_sink` (04) |
| No track configured or file missing | Thread returns silently | `NotConfigured`; "No music is set up." | None | `music::tests::missing_track_is_spoken` (04) |
| No output device | `try_default` error ignored | `DeviceUnavailable`; "I can't find a speaker." | Reopen the stream on the next play | `music::tests::no_device_is_reported` (04) |
| Decode error | Ignored | `InvalidInput`; "I can't play that file." | None | `music::tests::bad_file_is_reported` (04) |

## 3. News (RSS)

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Network down | 6 s per feed in sequence (up to 24 s busy), then an "unable" string | Parallel fetch, 4 s total budget; `Unreachable`; "I can't reach the news right now." | Serve the last good result if under 15 min old | `news::tests::offline_uses_cache_or_says_so` (10) |
| One feed slow | Blocks the rest | Partial results are fine | Drop the slow feed for this turn | `news::tests::slow_feed_is_skipped` (10) |
| Malformed XML | Garbage headline or channel title read aloud (unverified) | Parse items only; skip bad feeds | Skip | `news::tests::channel_title_is_not_a_headline` (10) |
| Empty result | Unclear reply | "No headlines right now." | None | `news::tests::empty_is_spoken` (10) |

## 4. Inbox (n8n webhook)

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| URL not configured | Error string | `NotConfigured`; "Inbox is not set up." | None | `inbox::tests::not_configured` (10) |
| Server never answers | No timeout; engine wedged, push-to-talk dead | 8 s timeout; `Timeout`; "The inbox is taking too long." | Turn ends; next press works | `inbox::tests::hung_server_times_out` (02) |
| Secret rejected (401, 403) | Error body may be spoken | `Unauthorized`; "The inbox rejected my key." | None | `inbox::tests::unauthorized` (10) |
| Malformed JSON | Raw text may be spoken | `InvalidInput`; "The inbox sent something I can't read." | None | `inbox::tests::malformed_body` (10) |

## 5. Google Meet

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| No credentials or token file | Error string | `NotConfigured`; "Google is not connected." | Doctor prints the consent steps | `meet::tests::not_connected` (10) |
| Token expiry misjudged | String compare of hand-built timestamps | Parse with `chrono`, compare instants, 60 s skew | Refresh | `meet::tests::expiry_uses_real_time` (03) |
| Refresh rejected (`invalid_grant`) | Error string | `Unauthorized`; "Google needs you to sign in again." | Health `down` until re-consent | `meet::tests::refresh_rejected` (10) |
| API hangs | No timeout; engine wedged | 8 s per call; `Timeout` | Turn ends | `meet::tests::hung_api_times_out` (02) |
| Busy flag stuck after a panic | Abort today; stuck under unwind | RAII guard releases on drop | Automatic | `meet::tests::busy_released_on_panic` (02) |
| Repeated request within cooldown | "Already on it" style reply | Same reply, cooldown from `Clock`, testable | Cooldown expires | `meet::tests::cooldown_uses_clock` (10) |

## 6. WhatsApp (neonize bridge)

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| DLL missing | Load error string | `NotConfigured`; "WhatsApp is not installed." Feature is off by default | None | `whatsapp::tests::dll_missing` (10) |
| Not linked (QR needed) | Writes `wa_link.txt` and opens it | `Unauthorized`; "WhatsApp needs linking. Open settings." QR shown by the shell | Health `not_linked` | `whatsapp::tests::unlinked_asks_for_link` (10) |
| Disconnected | Global flag | `Unreachable`; one reconnect attempt | Reconnect with backoff in the worker | `whatsapp::tests::reconnects_once` (10) |
| Contact unknown or ambiguous | Reply string | `InvalidInput`; "I don't know who that is." or a follow-up question | Follow-up slot | `whatsapp::tests::unknown_contact` (10) |
| Clipboard empty | "The clipboard is empty..." | Same sentence, typed `InvalidInput` | None | `whatsapp::tests::empty_clipboard` (10) |
| Worker stalls, queue grows | Unbounded channel | Bounded queue (8); `Busy`; send timeout 15 s | Worker restarted on next use | `whatsapp::tests::full_queue_is_busy` (10) |

## 7. n8n workflows

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Base URL unset | Falls back to a personal IP, then times out | `NotConfigured`; "Workflows are not set up." | None | `workflows::tests::no_base_url` (06) |
| Unknown workflow name | Reply string | `InvalidInput`; "I don't have a workflow called that." | None | `workflows::tests::unknown_name` (10) |
| Webhook 404 or 5xx | Error string | `Rejected`; "That workflow failed." | None | `workflows::tests::http_error` (10) |
| Timeout (sync 8 s, async 3 s) | Error string | `Timeout`; "That workflow is taking too long." | None; async runs are not retried | `workflows::tests::timeout` (10) |
| Non-JSON reply | Raw text may be spoken | Speak only a `speech` field; else "Done." | None | `workflows::tests::plain_text_reply` (10) |

## 8. Hub

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Anything | Nothing runs: the hub is not wired | Optional feature, off by default | Not applicable | `hub::tests::disabled_by_default` (19) |
| Hub unreachable | Not applicable | `Unreachable`; local features keep working; nothing is spoken unless the user asked the hub | Reconnect with backoff 1 to 60 s | `hub::tests::offline_does_not_block_turns` (19) |
| Duplicate event | `SEEN` set cleared at 500 | Bounded LRU of event ids | Drop | `hub::tests::duplicate_event_dropped` (19) |
| Token rejected | Not applicable | `Unauthorized`; health `down` | None | `hub::tests::unauthorized` (19) |

## 9. Screen read

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Capture fails | Error string | `Internal`; "I can't see the screen." | None | `screen::tests::capture_failure` (10) |
| Vision model missing | Error body | `NotConfigured`; "The vision model is not installed." | Doctor names the model | `screen::tests::model_missing` (10) |
| Model never answers | No timeout; engine wedged | 30 s budget; `Timeout` | Turn ends | `screen::tests::hung_model_times_out` (02) |
| Cancel during analysis | Result spoken after cancel (unverified) | Epoch check before speaking | Drop the result | `screen::tests::cancel_drops_result` (12) |

## 10. Ollama (LLM)

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Not running | "Please wait while I start it." Nothing starts | Policy from config: `start` (spawn `ollama serve`, wait up to 10 s) or `report` ("The language model is off.") | Health `down`; recheck every turn | `llm::tests::down_message_matches_policy` (19) |
| Model missing | Error text | `NotConfigured`; "The model is not installed." | Doctor prints the pull command | `llm::tests::model_missing` (19) |
| Cold start slow | 60 s blocking call | Stream tokens; say "One moment." after 2.5 s of silence | Continue | `llm::tests::slow_first_token_filler` (20) |
| Connection drops mid-answer | Error | Speak what arrived, then "I lost the connection." | None | `llm::tests::midstream_drop` (20) |
| Stale reachability in follow-ups | `neural` computed once | Checked per turn | Automatic | `turn::tests::llm_health_checked_each_turn` (12) |

## 11. Speech to text

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Model file missing | HUD stuck in `error` | `NotConfigured`; HUD error then idle; tray notice | Doctor fetches the model | `turn::tests::stt_error_returns_to_idle` (02) |
| Audio too short (under 0.1 s) | Skipped silently | Earcon or nothing, by config; HUD idle | None | `turn::tests::short_press_is_ignored` (12) |
| Hallucinated repeat | `dedup.rs` | Same, plus a no-speech check from VAD | Drop | `stt::tests::silence_yields_empty` (20) |
| Transcribe very slow | Blocks | 15 s budget; `Timeout`; "I didn't catch that." | Turn ends | `turn::tests::stt_timeout` (12) |
| Mic missing or changed | New stream per press; `eprintln!` lost | `DeviceUnavailable`; "I can't find a microphone."; health `down` | Reopen on device change | `audio::tests::device_loss_recovers` (20) |

## 12. Text to speech

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Kokoro model missing | Silent fall back to Piper, then WinRT | Same chain, but logged and shown as `degraded` in health | Automatic | `tts::tests::fallback_chain_reports_degraded` (14) |
| espeak-ng missing | Falls back to `simple_g2p` silently | Same, health `degraded` | Automatic | `tts::tests::g2p_fallback_is_reported` (14) |
| Piper process hangs | 550 ms idle rule, kill and respawn | Length-framed read where the build allows it; else keep the rule behind a named constant with a test | Respawn | `piper::tests::hung_child_is_replaced` (21) |
| No output device | Error ignored, silence | `DeviceUnavailable`; reply shown as text in the HUD | Reopen on next clip | `tts::tests::no_device_shows_text` (21) |
| Cancel during speech | Epoch poll every 20 ms | Same, plus barge-in by trigger | Stop within 50 ms | `turn::tests::cancel_stops_speech_fast` (12) |

## 13. Hotkey

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Hook install fails | Unknown, likely silent (unverified) | Exit code 70 with a log line | Supervisor restarts | `hotkey::tests::install_failure_is_fatal` (13) |
| Windows removes a slow hook | Push-to-talk dies silently (known risk of low-level hooks; unverified here) | Callback only posts a message; reinstall on session unlock and after 10 min without events | Automatic | `hotkey::tests::reinstall_is_idempotent` (13) |
| Modifier state desync after lock or UAC | Stuck or missing modifier (unverified) | Reset modifier state on session change | Automatic | `hotkey::tests::state_resets_on_session_change` (13) |
| Press while busy | Ignored until the worker returns, even after dismiss | State machine: press during speech is barge-in | Not applicable | `turn::tests::press_after_dismiss_starts_turn` (02) |

## 14. Wi-Fi presence policy

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| `netsh` output unparsable or localized | SSID empty, treated as away if a home list exists (unverified) | Use WLAN API events; unknown means "keep current state" | Next event | `presence::tests::unknown_keeps_state` (19) |
| No Wi-Fi adapter | Away forever if a home list exists | `presence.mode = "always"` default; doctor warns | None | `presence::tests::no_adapter_is_home` (19) |
| Unload during a live turn | Models unloaded every 5 s while away, no coordination | Presence posts an event; the turn machine unloads only when idle | Automatic | `turn::tests::unload_waits_for_idle` (19) |
| Empty home list | Means "always home" silently | Same, but explicit and shown in doctor | None | `presence::tests::empty_list_is_always` (19) |

## 15. Shell supervisor

| Failure | Today | Target | Recovery | Test |
|---------|-------|--------|----------|------|
| Engine exe not found | Candidate search, then silent | Tray error with the searched paths | Retry on user action | `SupervisorTests.MissingEngineIsReported` (18) |
| Crash loop | Three restart paths can race | One supervisor; backoff 1, 2, 5, 15, 60 s; stop after 5 restarts in 10 min | Tray "Restart" action | `SupervisorTests.BackoffAndGiveUp` (18) |
| Engine hung | `Watch-Kuro.ps1` heartbeat check | Supervisor reads heartbeat age from `/status`; engine self-watchdog exits with code 70 | Restart | `SupervisorTests.StaleHeartbeatRestarts` (18) |
| Port in use | Engine aborts, restart loop | Engine keeps push-to-talk alive, retries the bind every 5 s, exit code never used for this | Automatic when the port frees | `engine::tests::bind_conflict_keeps_ptt` (02) |
| Version skew | Undetected | Handshake; shell shows "Engine and shell versions differ" | User updates | `ContractTests.MajorMismatchIsRefused` (15) |
| Two shells | Both run | Single-instance mutex; second instance focuses the first | Automatic | `SupervisorTests.SecondInstanceExits` (18) |
