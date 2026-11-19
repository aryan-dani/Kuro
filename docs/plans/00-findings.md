# 00 Findings

Verified against commit `f661122` on `main` (2026-11-14).

Evidence standard: every verdict below comes from reading the code. Nothing in this
file was runtime-tested in this session unless the row says so. Rows marked
"unverified at runtime" are predictions from code reading and must be confirmed by the
regression test named in the fix phase.

Verdicts: CONFIRMED (the claim is true), PARTIAL (true with a correction), WRONG (the
code says otherwise). Paths are relative to the repo root; Rust paths omit the
`kuro-rs/crates/` prefix.

Companion file: [00-failure-modes.md](00-failure-modes.md) holds the per-integration
failure-mode tables.

## A. Structural problems (prompt section 3)

| ID | Claim | Verdict | Evidence |
|----|-------|---------|----------|
| S1 | Adding one tool touches many places | CONFIRMED | `kuro-tools/src/lib.rs` `ToolRegistry::default_registry()`; `kuro-router/src/semantic.rs` `Intent` (14 to 36), `EXEMPLARS` (52 to 156), tier-1 map (158 to 200), `intent_tag` and `intent_from_tag` (about 495 to 544); `kuro-router/src/phrases.rs` `execute_intent` (153 to 201), `legacy_try_handle` (203 to 273), `is_*` predicates, help text (twice); `kuro-stt/src/lib.rs` `PROMPT_BASE`; `docs/NATIVE_REWRITE_SPEC.md` sections 5, 6, 18. Missed by the prompt: the `load_env_file` allowlist in `kuro-engine/src/main.rs` (92 to 106) and `.env.example`. Total: 9 to 11 edit sites in 4 crates. |
| S2 | `phrases.rs` duplicates routing; ordering implicit | CONFIRMED | Replies and tool calls exist in both `execute_intent` and `legacy_try_handle`. `route_command` calls `try_handle` up to three times. Order is the order of the `if` chain. |
| S3 | `main.rs` is one huge function | CONFIRMED | `main()` spans lines 87 to 621 (535 lines), about 25 cloned `Arc`/`Rc` handles, follow-up loop at 417 to 585 inside the worker closure created in `on_up`. No seam for fakes. |
| S4 | Duplicated helpers | CONFIRMED, with corrections | Four `normalize` functions, not three: `phrases.rs:26`, `n8n.rs:44`, `whatsapp.rs:31`, `meet.rs:52`. Three resamplers: `kuro-audio/src/lib.rs:62` `resample_linear`, `kuro-tts/src/kokoro.rs:348`, `kuro-stt/src/worker.rs:139` `resample_to_16k`. Mask helpers in `kuro-core/src/config.rs` (`mask_number`, `mask_id`) and `KuroShell.Core/ShellConfig.cs` (unused by the UI). DTOs in `kuro-ipc/src/lib.rs` `event_frame` and `KuroShell/KuroShell/EngineJson.cs` with no contract test. |
| S5 | Tests rely on thread-local hooks and globals | CONFIRMED | `kuro_tools::set_executor` (`thread_local! HOOK`), n8n `LOADED/BASE/TOKEN/REGISTRY`, `HOME_COOLDOWN_UNTIL`, `MEET_BUSY`, `MEET_COOLDOWN_UNTIL`, `MUSIC`, `EMBED`, hub `SEEN`, WhatsApp `STATE/QR_FLAG/READY_FLAG`. `fire_daddys_home` spawns threads, which bypass the thread-local hook and run real plug and music code. That is what broke CI on 2026-11-13. |
| T1 | Personal data in code and docs | PARTIAL | Phase 05 scrub: `kuro-engine/src/main.rs` no longer sets a Tailscale default (no `100.78.60.50` at startup). Remaining hotspots called out in prior audit: `kuro-tools/src/hub.rs:22`, `.env.example` (25, 29), `README.md:5`, `deploy/n8n/docker-compose.yml`, `docs/N8N.md`, tools tests, `192.168.1.240`, `kuro-server`, `docs/CLOUD_INBOX.md`, `C:\Users\dania\` in `bench/results.md`, example contacts in tests and docs, Telegram and Groq in `workflows/`, the song filename in `kuro-tools/src/music.rs` `CANDIDATES`, "sir" in replies (frozen until honorific profile work), the owner mishearing table in `phrases.rs` `normalize`, `PROMPT_BASE`, mic token `"s23"` in `kuro-audio/src/lib.rs`. `docs/NATIVE_REWRITE_SPEC.md` and `workflows/kuro_daily_digest.json` scrubbed in phase 05. |
| T2 | Brand palette and fonts are equality invariants | CONFIRMED | `scripts/Test-KuroInvariants.ps1` lines 81 to 104 require literal substrings of `DesignTokens.cs` (for example `245, 244, 238`) and three font files. |
| T3 | Docs drift | CONFIRMED | `voices/README.md` is entirely XTTS. `.env.example` has an XTTS comment. `NATIVE_REWRITE_SPEC.md` says Python is removed (1.3) and that Python is the source of truth (16). Spec TTS wire values omit `kokoro`; `kuro-core/src/config.rs` includes it. `ShellConfig.Pages` lists 8 pages; `MainWindow` has 5 different ones. Perf docs are Python-era. Also found: `HUD_DESIGN.md` says 11 bars, the spec says 9, the code sends 11. `docs/N8N.md` references a script that does not exist. `docs/SHELL.md` says "stay in Python". `DESK_POWER.md` and `docs/WIFI_WATCH.md` describe `Watch-KuroWifi.ps1` as the autostart path while `Install-KuroAutostart.ps1` unregisters that task as legacy. The installer comment says 90 s; the code uses 15 s. |
| T4 | Licensing | CONFIRMED | No `LICENSE` file exists (Cargo metadata says MIT). OFL texts exist for all three fonts (`OFL.txt` is the Poppins one; the name is ambiguous). Third-party audit is in [04-repo-hygiene.md](04-repo-hygiene.md). The song is referenced by filename only. |
| T5 | `.gitignore` `*.png` swallows manifest tiles | PARTIAL | The four tiles the manifest needs are already tracked, so nothing is lost today. Any new PNG under `KuroShell/` would be ignored silently because the only exception is `assets/branding/*.png`. |
| C1 | CI and guardrails gaps | CONFIRMED | `.github/workflows/ci.yml` has one `windows-latest` job. `Test-KuroGate.ps1` `Stop-LockingKuro` runs `Stop-Process -Force` on `KuroShell` and `kuro-engine`, deletes `kuro.lock`, and at the end runs `Start-Kuro.ps1` even when Kuro was not running before (unless the `DISABLED` flag exists). `.githooks/pre-commit` runs the whole gate including the live smoke. Invariants are regex over source. Absent: CODEOWNERS, PR and issue templates, CHANGELOG, SECURITY, CODE_OF_CONDUCT, ARCHITECTURE, AGENTS.md, `.cursor/rules`, `rust-toolchain.toml`, `deny.toml`, dependency update config. |

## B. Bugs and fragility (prompt section 4)

"Fix" names the phase file that owns the fix and its regression test.

| # | Claim | Verdict | Evidence | Fix |
|---|-------|---------|----------|-----|
| 1 | Music controls are fake; stream dropped | CONFIRMED (unverified at runtime) | `kuro-tools/src/music.rs` `play_file`: `let (_stream, handle) = OutputStream::try_default()`, then `sink.detach()` and return, which drops `_stream`. `pause_song`, `resume_song`, `stop_song` only flip flags in `MUSIC`. | phase 04 |
| 2 | `panic = "abort"` defeats `catch_unwind` | CONFIRMED | `kuro-rs/Cargo.toml` release profile. `ToolRegistry::execute` and `try_handle` wrap `catch_unwind`. | phase 02 |
| 3 | Token RNG is not cryptographic | CONFIRMED, worse | `kuro-core/src/token.rs` `random_hex` is an LCG that emits `state & 0xff` per step, so the low 8 bits of the seed determine the whole token: 256 possible tokens. The shell uses `RandomNumberGenerator.GetBytes(32)` when it creates the token first (`EngineSupervisor.EnsureToken`). | phase 03 |
| 4 | `speak_sapi` injection | PARTIAL | `kuro-tts/src/lib.rs:373` interpolates `{text:?}` into a PowerShell script. No caller exists. Dead code; delete it. | phase 03 |
| 5 | Tuya fixed nonces | CONFIRMED | `tuya.rs:207` GCM nonce is `[1..=12]`, sequence number is always 1. `plug.rs:87` local nonce is `1..16`. | phase 03 |
| 6 | `/say` executes real tools | CONFIRMED | `kuro-ipc/src/lib.rs` `/say` calls `kuro_router::try_handle(text)`. | phase 03 |
| 7 | Meet token expiry compare is fragile | CONFIRMED | `meet.rs` 285 to 298 `token_expired` compares hand-built ISO strings lexicographically. | phase 03 |
| 8 | Hub unwired | CONFIRMED | `voice.rs` passes `None` as the hub result. `hub::command`, `list_tools`, `execute_tool`, `health`, `ws_url`, `handle_event` have only test callers. The hub branch of `route_command` is unreachable. | phase 19 |
| 9 | `load_env_file` allowlist is incomplete | CONFIRMED | 13 keys at `main.rs` 92 to 106. Missing at least `KURO_HUB_URL`, `KURO_HUB_TOKEN`, `N8N_INBOX_WEBHOOK_URL`, `N8N_WEBHOOK_SECRET`, `KURO_TIMING`, `KURO_LLM_MODE`, `OLLAMA_VISION_MODEL`, `KURO_HOME_SSID`. | phase 07 |
| 10 | Ollama-down message promises a start that never happens | CONFIRMED | `voice.rs` sets "Neural engine is off. Please wait while I start it." and starts nothing. `ollama_up()` runs once at `main.rs:393`; follow-up turns reuse the stale value. | phase 19 |
| 11 | Live/final preference dead; follow-up after every reply; partial ignored; beam | CONFIRMED | `turn.run(&transcript, &transcript, false, false, ...)` at `main.rs:399`. `follow_up_for` has only test callers. The follow-up loop reopens the mic after every non-cancel reply. `worker::transcribe(_, _, _partial, _, _device)` ignores both. `beam_size` defaults to 1 and is never passed to the worker (`best_of: 1`, greedy). `initial_prompt(&[], &[])` passes no aliases or contacts. | phases 19, 20 |
| 12 | Wi-Fi stub, polling, empty list | CONFIRMED | `kuro-wifi/src/lib.rs` `register_notifications` passes a `None` callback and is never called. `main.rs` 147 to 166 polls `netsh` every 5 s and calls `unload_speech()` on every poll while away, with no turn coordination. `should_run` returns true when the home list is empty. | phase 19 |
| 13 | `tts_first_audio` recorded as 0.0 | CONFIRMED | `voice.rs:75` `timing.once("tts", "tts_first_audio", 0.0)`. Follow-up turns record nothing because `finish` cleared `active`. Two separate `Timing` instances exist. | phase 19 |
| 14 | Escape handled in three places | CONFIRMED | Engine hook posts `WM_KURO_ESC` only while PTT is held; shell `EscapeHook` has a 350 ms suppression window (lines 55, 86); the HUD handles it too. | phase 12 |
| 15 | Two supervisors; `Thread.Sleep` in handler; forced CPU | CONFIRMED, three restart paths | `EngineSupervisor` `Exited` handler (`Thread.Sleep` at 101 and 263), the 15 s watchdog in `App`, and `scripts/Watch-Kuro.ps1`. Env forced at `EngineSupervisor.cs` 206 to 209. | phase 18 |
| 16 | Piper idle gap; stream per clip; no barge-in | CONFIRMED | `piper.rs` 137 to 208 (`idle = 550 ms`, kill on failure). `kuro-tts/src/lib.rs:350` opens a `rodio::OutputStream` per clip. Stop is epoch polling every 20 ms only. | phases 20, 21 |
| 17 | `ConversationBuffer.Observe` drops repeats | CONFIRMED | `KuroShell.Core/ConversationBuffer.cs` drops an identical consecutive utterance and an identical reply. | phase 19 |
| 18 | WhatsApp bridge fragility | CONFIRMED | `wa_neonize.rs`: unbounded `mpsc::channel`, `Box::leak` at 259 to 264, `show_qr` writes `wa_link.txt` and opens it with `cmd /C start`, statics `STATE`, `QR_FLAG`, `READY_FLAG`. | phase 10 |
| 19 | String prefixes as control flow | CONFIRMED | `phrases.rs` `power()` (`starts_with("success")`), `speakable()`, `news_to_speech()` (`"unable"`), `meet.rs:227`. | phases 09, 10 |
| 20 | Busy flags can stick | PARTIAL | Normal and early-return paths call `release_meet`. A panic would skip it in an unwinding build; in release the process aborts instead. Becomes real once phase 02 switches to unwind, so the guard must be RAII. | phase 02 |
| 21 | Dead bench scripts | CONFIRMED | `scripts/bench/*.py` import `timing`, `phrases`, `stt`, `power` from the deleted `legacy/python`. | phase 05 |

## C. New discoveries

| ID | Finding | Evidence | Fix |
|----|---------|----------|-----|
| N1 | Settings are write-only. `PUT /config` changes `EngineConfig`, but the voice path never reads it; `llm_mode` is the literal `"fallback"` at the call site; `paused` is cosmetic; nothing is persisted. | `kuro-core/src/config.rs`, `kuro-ipc/src/lib.rs`, `main.rs:399` | phase 07 |
| N2 | STT backend selection and Legion gating are dead code (always CPU), yet an elevated scheduled task keeps writing `legion_mode.json`. | `kuro-stt/src/lib.rs` `select_backend`, `ready_for_heavy`; `worker.rs` `_device`; `scripts/Write-KuroLegionMode.ps1` | phase 19 |
| N3 | Network calls without timeouts can wedge the engine: inbox webhook, screen-read vision call, Meet OAuth and create-space. The worker holds `busy`, so push-to-talk is ignored until the call returns. | `feeds.rs` `inbox_run`, `screen_run`; `meet.rs` 267 to 396 | phase 02 |
| N4 | `/dismiss` and Escape cancel the epoch but do not clear `busy`; the next press is ignored until the worker returns. | `kuro-ipc/src/lib.rs` `/dismiss`; `main.rs` escape handler | phase 02 |
| N5 | A port conflict kills the whole engine, hotkey included: `.expect("bind")` and `.expect("tokio")` in the API thread plus `panic = "abort"`. | `main.rs` API thread | phase 02 |
| N6 | Follow-up conversation exists only when the API is on (the loop sits inside `if let Some(app) = state.as_ref()`), and follow-up turns are untimed. | `main.rs` 417 to 585 | phase 12 |
| N7 | STT error leaves the HUD in "error" and never returns to idle. | `main.rs` worker error path | phase 02 |
| N8 | Mic ranking is hardcoded with owner tokens (`"s23"` banned, `"realtek"` preferred). A new cpal stream opens per press, so there is no pre-roll (first-syllable clipping is likely; unverified). | `kuro-audio/src/lib.rs` `open_input` | phases 06, 20 |
| N9 | `eprintln!` diagnostics vanish under `windows_subsystem = "windows"`. | `kuro-audio`, tests | phase 02 |
| N10 | Three different default Ollama models: `qwen2.5:7b-instruct` (engine, shell, `Start-Kuro.ps1`) and `llama3.2` (`.env.example`). | `voice.rs`, `EngineSupervisor.cs`, `.env.example` | phase 07 |
| N11 | `route_command` can run the semantic embed twice for an utterance that falls through to the LLM. | `phrases.rs` `route_command` | phase 09 |
| N12 | The shell has no single-instance guard. | `App.xaml.cs` | phase 18 |
| N13 | `/shutdown` does not exit the process outside gate mode. | `kuro-ipc/src/lib.rs`, `main.rs` gate branch | phase 18 |
| N14 | Tuya success is unverified: `write_frame` returns `Ok` without reading a response, so "Desk power updated" can be spoken when nothing happened. Default target is `127.0.0.1` when no IP is known. `control_smart_bulb` shares the plug's device id and no phrase reaches it. | `tuya.rs`, `plug.rs` | phase 10 |
| N15 | The last contact cannot be deleted (`contacts_ok` rejects an empty map). | `kuro-ipc/src/lib.rs` | phase 19 |
| N16 | RSS parsing splits on `<title>`, which likely reads the channel title as a headline (unverified at runtime). The feed list is hardcoded. | `feeds.rs` `titles()` | phase 10 |
| N17 | `.cargo/config.toml` forces `target-cpu=x86-64-v3` and MSVC AVX2 `CFLAGS` for every target. This is an undocumented AVX2 hardware requirement, and the `[env]` block breaks non-MSVC builds of C dependencies. | `kuro-rs/.cargo/config.toml` | phases 01, 05 |
| N18 | Model downloads have no checksums. | `scripts/Download-*.ps1` | phase 08 |
| N19 | Tracked files contain em dashes, including one introduced on 2026-11-13 at `phrases.rs` line 9. | `kokoro.rs`, `phrases.rs`, `meet.rs`, spec, scripts, docs | phase 01 |
| N20 | Running tests on a machine with real `TUYA_*` variables can send real plug commands through spawned threads (unverified at runtime). | `phrases.rs` `fire_daddys_home` | phase 00 |
| N21 | The shell forces engine env at spawn (`KURO_API=on`, `KURO_HUD_SHOW_TEXT=1`, `KURO_TTS_PLAY=1`, `KURO_STT_BACKEND=cpu`), so behavior depends on who launched the engine. | `EngineSupervisor.cs` 206 to 209 | phase 07 |
| N22 | Dead or unused code: `prefer_user_text`, `looks_like_daddys_home`, `is_local_only`, `follow_up_for`, `beam_size`, `select_backend`, `ready_for_heavy`, `speak_sapi`, all hub functions, `register_notifications`, `ShellConfig.IsAllowed/MaskNumber/MaskId/Pages`, `LevelSmoother`, `Spring`, `ResultHoldMs`, `scripts/Watch-KuroWifi.ps1`, bench Python. | grep for callers | phases 05, 19 |
| N23 | Layering is inverted. `kuro-core` depends on `ort` and `windows-sys`. `kuro-router` depends on `kuro-tools` and executes tools. `kuro-ipc` depends on router, tools, and tts. | crate `Cargo.toml` files | phases 09, 12 |
| N24 | Tests skip silently when models or espeak are absent, so green CI does not prove the model paths. | `kokoro.rs` tests, `semantic_router.rs` `onnx_cases_optional`, `kuro-tts/tests/cache.rs` | phase 01 |
| N25 | `ToolRegistry::default_registry()` rebuilds all 8 tools on every call. | `kuro-tools/src/lib.rs` | phase 09 |
| N26 | `pid_alive` treats any process whose name contains "kuro" as a live engine. | `kuro-core/src/lib.rs` | phase 18 |

## D. Where the prompt and the code disagree

The code wins in each case.

1. "Three `normalize` functions": there are four.
2. "Two supervisors": there are three restart paths.
3. "`speak_sapi` possible injection": the code exists but nothing calls it.
4. "Time-seeded LCG, not cryptographic": it is weaker than that, 256 possible tokens.
5. "`beam_size` defaults to 1 though docs say 5": it defaults to 1 and is not applied at
   all.
6. "`.gitignore` swallows required tiles": not today, only future PNGs.
7. "`MEET_BUSY` can stick": not on any current path in a release build.
8. Section 1 calls the third TTS engine "WinRT SAPI". The live path is WinRT
   `SpeechSynthesizer` (`kuro-tts/src/sapi.rs`); the PowerShell SAPI path is dead.
9. Section 8 performance numbers (STT 350 ms, Kokoro 200 ms, router 2.4 ms, Ollama
   1.8 s) cannot be reproduced from the repo: the perf docs are Python-era. Treat them
   as owner-reported and unverified until phase 00 measures them.
10. Suggested phase 1 lists music as a critical fix, while leaning 10 limits that stage
    to crashers and security. Music is kept early as its own small phase (04).

## E. What was not verified

- Any runtime behavior (audio, hotkey, Tuya, WhatsApp, Meet, Ollama).
- Third-party license texts (listed from memory in file 04; confirm at phase time).
- Whether Task Scheduler restart-on-failure works for the WinUI shell.
- Whether `kuro-core` compiles on Linux today.
- Gate timings (measured in phase 01).
