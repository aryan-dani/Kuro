# Native rewrite specification

This document is the Phase 0 / Phase 1 parity contract for replacing the Python engine with a native Rust `kuro-engine` while keeping `KuroShell` as the WinUI surface. It records what the repository does today. Where a rewrite brief and this code disagree, this document wins until an explicit product acceptance changes it.

No secrets, tokens, or credential values appear here. No emoji appear in this document.

---

## 1. Purpose and parity locks

### 1.1 Goal

Ship a performance rewrite: lower idle RAM, lower VRAM when dormant, lower time-to-first-audio, and remove the PyTorch / FastAPI process. Behavior, phrase coverage, tool safety, localhost API, hub offloads, and shell visuals must match the current product.

### 1.2 Hard locks

1. Windows 10/11 x64 only.
2. Push-to-talk remains hold `Ctrl+Alt+Space` (default `KURO_HOTKEY=ctrl+alt+space`), owned by the engine, not the shell. Today: `pynput` in `main.register_push_to_talk`.
3. Always-on-top bottom mark: non-activating, bottom of the work area. State transitions (`idle` / `listening` / `thinking` / `speaking` / `error` and related HUD labels) must never move or jitter its screen coordinates. Place once on init and on display-change events.
4. Every tool / action route returns a speakable string and must not panic or raise unhandled exceptions to the caller (`tools.ToolRegistry.execute`).
5. Brand and copy:
   - Settings / paper UI colors: Paper `#F5F4EE`, White `#FFFFFF`, Ink `#141414`, Muted `#52514E`.
   - Settings geometry: square corners (`CornerRadius` / `RadiusRow` / `RadiusCard` = 0).
   - Fonts: Space Grotesk (titles), JetBrains Mono (labels), bundled under `KuroShell/KuroShell/Assets/Fonts`.
   - Capsule: black bottom pill (existing shell). Chat card under the pill: Poppins, radius 22, panel `#0A0A0A`, ink `#F0F0F0`, muted `#A3A3A3`. Do not restyle these as part of the rewrite.
   - No emoji in UI copy, logs, comments, or documentation.
6. Secrets and local state stay under `%LOCALAPPDATA%\Kuro` or ignored files. Never commit `.env`, `api_token`, `credentials.json`, `token.json`, Tuya cache, or timing CSVs.
7. Hub default base `http://100.78.60.50:18200` for desk rituals when configured. Local LAN Tuya remains the laptop fallback for the plug. Email-to-Telegram triage stays on n8n Cloud (`N8N_INBOX_WEBHOOK_URL`). Self-hosted n8n voice workflows use `KURO_N8N_BASE_URL` (default `http://100.78.60.50:5678/webhook`).
8. Full phrase catalog, tools, Meet, WhatsApp, n8n registry, Ollama fallback, timing CSV shape, dismiss turn-epoch, follow-up listen after `?`, and early daddy-home fire must be preserved.
9. TTS wire values stay `auto|xtts|piper|pyttsx3`. Product acceptance for `kuro-engine.exe`: `auto` and `piper` use native Rust ONNX (`KURO_PIPER_MODEL` via `ort` or `sherpa-onnx`, sentence streaming, clip cache) on AC and battery, with no resident Python or PyTorch. `pyttsx3` maps to Windows SAPI / WinRT `SpeechSynthesizer`. Forced `xtts` may call an on-demand external helper or fall back to native Piper ONNX. The engine process stays free of a resident Python/PyTorch runtime.
10. Hub timeouts stay as coded: command 8s, tools list 5s, tool exec 30s, health 3s. A 2s hub timeout is not the current contract.
11. Localhost wire for `KuroShell` stays `http://127.0.0.1:8765` with header `X-Kuro-Token`. A named pipe (`\\.\pipe\KuroEngine`) may be additive later; it does not replace the TCP contract for the shell.
12. News via the phrase router is local RSS (`tools/news.py`). The hub is not the only news path.
13. NPU bench stack is `requirements-client.txt` (OpenVINO 2026.4 line). There is no `requirements-npu.txt` in this repo. Production STT deps are in `requirements.txt` (`faster-whisper` + CUDA helpers).

### 1.3 Python remains until cutover

**Status (2026-11-13):** the Python engine under `legacy/python` has been removed. `kuro-engine.exe` is the only runtime. Optional offline helpers may remain under `scripts/bench/*.py`.

---

## 2. Process map

```text
User (Ctrl+Alt+Space)
        |
        v
+------------------+     optional      +------------------+
| Engine           | <---------------- | KuroShell        |
| (Python today;   |  127.0.0.1:8765   | WinUI 3 / .NET 10|
|  kuro-engine     |  X-Kuro-Token     | tray + capsule   |
|  later)          |  WS /events       | + settings       |
+------------------+                   +------------------+
   |        |        \
   |        |         \-- Ollama 127.0.0.1:11434 (lazy)
   |        |
   |        +-- Hub  http://100.78.60.50:18200  (desk rituals when token set)
   |        +-- n8n self-host  ...:5678/webhook  (named workflows)
   |        +-- n8n Cloud inbox webhook
   |
   +-- Local: WASAPI/mic, STT, TTS, pygame music, tinytuya plug,
              WhatsApp session, Google Meet token, RSS news
```

- Shell does not own STT, TTS, phrase routing, or the hotkey (`docs/SHELL.md`).
- Engine API binds only when `KURO_API=on`. Shell sets `KURO_API=on` and `KURO_HUD_SHOW_TEXT=1` when it spawns the engine.
- Headless: `scripts/Start-Kuro.ps1` can run the engine without the shell; Python tray + Tk overlay still apply when API is off.

---

## 3. Voice-turn state machine

### 3.1 Push-to-talk

1. Hold modifiers + Space begin capture (`start_push_to_talk`).
2. Mic levels push to the HUD; optional partial STT captions live transcript.
3. Early match for daddy's home may fire plug + song before key release.
4. Release stops capture, writes a temp WAV, runs `process_audio_file`.
5. Final STT; prefer live transcript when final is empty or when live matches a phrase and final does not.
6. Route (section 4), speak, optional `after_speak`, then Idle.
7. If the spoken reply ends with `?` and the turn epoch is still live, start follow-up listen (~0.4s delay, open mic ~7s).

### 3.2 Turn epoch (dismiss cancel)

Integer `_turn` in `main.py`. Escape and `POST /dismiss` call `_force_unblock`, which:

- increments `_turn`
- clears follow-up flag
- sets `cancel.event` and speech cancel
- stops mic / `sounddevice`
- clears busy / PTT
- shell `force_hide` after dismiss

Each voice turn snapshots `epoch = _turn` and checks `_live(epoch)` after STT, after routing, before transcript / state updates, before `_speak`, before `after_speak`, and before starting follow-up. `_mark_speaking` and `_follow_up_listen` take the same epoch. `_speak(epoch=...)` must not clear a cancel that belongs to a newer epoch. Hub / non-turn speech may omit epoch.

A closed capsule must not speak again or reopen from a stale `?` follow-up.

### 3.3 HUD states

Overlay / API string states: `hidden`, `listening`, `thinking`, `speaking`, `dormant`, `error`, `ready`, `done`.

Product flow for a normal turn: Listening (bars) -> Thinking (shimmer until first TTS audio) -> Speaking -> Done or Listening (follow-up) -> hide.

Quiet / short hold: Quiet or Hold, then hide.

---

## 4. Routing order

```text
STT text
  -> phrases.try_handle   (local first; never waits on hub or Ollama)
  -> if miss and hub configured: POST /v1/command
       handled / duplicate / reply / client_actions
       if hub unreachable:
         music-only phrases: try local again
         hub desk phrases (power / goodnight / daddy home / news): "I can't reach the hub."
         else fall through
  -> phrases.try_handle again (inbox / remaining local)
  -> LLM:
       KURO_LLM_MODE=off  -> offline no-match spoken line; no Ollama
       fallback / always  -> lazy Ollama; if serve is down, speak
                            "Neural engine is off. Please wait while I start it."
                            then start and answer
```

Local desk phrases never wait on the hub. Meet, WhatsApp, workflows, and inbox stay on the laptop even when the hub is up.

---

## 5. Phrase catalog (`phrases.py`)

### 5.1 Normalization

Lowercase, strip punctuation to spaces, collapse whitespace. Replacement table includes (non-exhaustive; see `_normalize`):

- `daddy's` / `daddys` / `dadi` -> `daddy`
- `wake up` / `wake-up` / `woke up` -> `wakeup`
- `good night` -> `goodnight`
- home ASR fixes (`his home`, etc.) -> `daddy home`
- `what's` / `what s` -> `whats`
- `moniter` / `monitors` -> `monitor`; `extention` -> `extension`
- `monitor of` / `screen of` / ... -> corresponding `... off`
- `lets off` / `let s off` / `next off` -> `desk off`
- `bob it off` -> `power off`
- `urgent man` / `urgent men` -> `urgent mail`

Whole-word match for `on` / `off` so `monitor` is not treated as `on`.

### 5.2 Match order in `try_handle`

All matched phrase replies use `force_fast_tts` unless noted.

| Route | Triggers (summary) | Action / reply |
| --- | --- | --- |
| Cancel | exact: `cancel`, `never mind`, `nevermind`, `forget it`, `stop`, `abort` | `Alright.` |
| Thanks | contains `thank you`, `thanks`, `cheers`, `appreciate it` | `Anytime.` |
| Help | `help`, `commands`, or capability phrases | Desk / music / news / inbox / Meet / WhatsApp / workflows help line |
| Identity | `who are you`, `what are you`, `your name`, `whats your name` | `Kuro. Local assistant. Phrases first, model last.` |
| Time | time phrases; not date-only | Local clock string |
| Date | date / day phrases | Local date string |
| Pause | `pause` + music words, or short pause forms | `music.pause_song()` |
| Resume | resume / unpause / play music forms | `music.resume_song()` |
| Stop music | stop/close/kill + song/music/track/clash/media | `music.stop_song()` |
| Good night | `goodnight`, good+night, `night`, `sleep`, `bedtime` | Stop music; plug scene `good_night`; `Desk off. Good night, sir.` |
| Daddy's home | wakeup+home, daddy+home, or wakeup+daddy | Parallel plug `daddy_home` + `play_should_i_stay_or_should_i_go`; `Welcome home, sir.` Duplicate in flight: `Already on it, sir.` |
| Power off | off phrases / turn off / desk keywords + off; never promote when only on | Stop music; scene `power_off`; `Desk off.` |
| Power on | on phrases / turn on / desk keywords + on; whole-word off wins | Scene `power_on`; `Desk on.` |
| News | news / headlines / brief me / ...; not inbox | `get_news` topic tech/world/business/ai/sports/general; up to 3 spoken headlines |
| Meet | `tools.meet.match_phrase` | `make_meet` |
| Inbox | urgent mail / check inbox / deadlines / ... | modes `deadlines` / `urgent` / `summary` via `check_inbox` |
| WhatsApp | `tools.whatsapp.match_phrase` | `send_whatsapp` |
| Workflow | `tools.n8n.match_phrase` | `run_workflow` |
| Workflow hint | `n8n` / `workflow` / automation+verb without registry hit | Do not escalate to LLM; unknown-workflow spoken line |

Desk keywords for power: `light`, `lights`, `lamp`, `plug`, `charger`, `monitor`, `screen`, `display`, `desk`, `board`, `extension` (whole word; `board` must not match inside `clipboard`).

Plug failures speak short error lines (`I couldn't reach the plug.` or the tool error when short). Meet / WhatsApp / workflow failures: fixed spoken failure lines.

`matches(text)` is side-effect free and lists the same predicates for live-vs-final STT preference.

---

## 6. Tool catalog

Auto-discovered modules under `tools/` with `NAME`, `DESCRIPTION`, `PARAMETERS`, `run(args) -> str`.

| Name | Role | Notes |
| --- | --- | --- |
| `control_smart_plug` | Desk board | Scenes: `daddy_home`, `good_night`, `power_on`, `power_off`. tinytuya. Cache `%LOCALAPPDATA%\Kuro\tuya_endpoint.json`. Env default version `3.3` (`.env.example` may show `3.5`). |
| `control_smart_bulb` | Bulb | LLM tool: on/off/brightness/color/scene. Not a first-class phrase route. |
| `get_news` | Headlines | Local RSS: The Verge, Hacker News, BBC World, NPR. |
| `check_inbox` | Cloud mail triage | n8n Cloud webhook. Local-only vs hub merge (`brain._LOCAL_ONLY`). |
| `make_meet` | Google Meet | Token under `%LOCALAPPDATA%\Kuro\google\token.json`. May WhatsApp the link. |
| `send_whatsapp` | WhatsApp send | Contacts `%LOCALAPPDATA%\Kuro\contacts.json`; session `%LOCALAPPDATA%\Kuro\wa_session.db`. Sources: `clipboard`, `browser_url`, `text`. |
| `run_workflow` | Self-hosted n8n | Registry `%LOCALAPPDATA%\Kuro\n8n_workflows.json`. Header `X-Kuro-Token`. Body `{"text","source":"kuro"}`. Modes sync/async. |
| `read_screen` | Vision | Screenshot + `OLLAMA_VISION_MODEL` (default `qwen3-vl:8b`). Local-only. |

Music is not a `tools/` module. Python uses `music.py` (pygame mixer). Native acceptance: `rodio` / `cpal` with the same `pause_song`, `resume_song`, `stop_song`, and `play_should_i_stay_or_should_i_go` outcomes. Track files: `should_i_stay_or_should_i_go.mp3` / `.wav` (or `DADDY_HOME_SONG_PATH`).

Native tool acceptance: `control_smart_plug` (LAN Tuya TCP 3.3 / 3.4 / 3.5 and `tuya_endpoint.json`), `control_smart_bulb`, `get_news` (the same four RSS feeds), `check_inbox`, `run_workflow`, `read_screen` (Win32 capture, base64 image, `OLLAMA_VISION_MODEL`), and `make_meet` (OAuth refresh from `google\token.json` plus the Google Meet API) are Rust. `send_whatsapp` matches phrases, contacts, and clipboard or browser URL in Rust, then sends through a one-shot `neonize` subprocess that exits when the send finishes so idle RAM stays at zero. `wa_session.db` stays the session store.

Hub client music `client_actions`: `pause`, `resume`, `stop`, `play` with `track=daddy_home`.

---

## 7. Hub wire format (`hub_client.py`)

Base: `KURO_HUB_URL` default `http://100.78.60.50:18200`. Enabled only when URL and `KURO_HUB_TOKEN` are both set.

Auth: `Authorization: Bearer <token>`, `Content-Type: application/json`.

| Method | Path | Body / notes | Timeout |
| --- | --- | --- | --- |
| POST | `/v1/command` | `{"text","source":"laptop","early":bool}` -> `handled`, `duplicate`, `reply`, `client_actions` | 8s |
| GET | `/v1/tools` | list of Ollama-shaped tool specs; hub wins shared names except `read_screen`, `check_inbox` | 5s |
| POST | `/v1/tools/{name}` | `{"arguments":{}}` -> `result` string | 30s |
| GET | `/health` | reachability | 3s |
| WS | `/v1/events` | ws or wss from http/https base | reconnect with backoff |

Inbound event types: `speak` (`payload.text`), `notify` (`payload.text`), `client_action` (music ops). Ignore `pong`. Ack: `{"type":"action_result","event_id","ok","detail"}`. Deduplicate event ids (cap ~500).

Self-hosted n8n (`KURO_N8N_*`) is a separate client from the hub.

---

## 8. Localhost engine API (`engine_api.py`)

### 8.1 Binding

- Host `127.0.0.1`, port `KURO_API_PORT` (default `8765`).
- Enabled only if `KURO_API` is `on` / `1` / `true` / `yes`.
- Token file: `%LOCALAPPDATA%\Kuro\api_token` (64 hex chars created once). Refuse to listen if missing/empty.
- Header: `X-Kuro-Token` (HTTP and WebSocket). Bad token: HTTP 401; WebSocket close code 1008.
- Never log request bodies or the token.

### 8.2 HTTP routes

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/status` | `{ok, paused, state, api, logs}` |
| GET | `/config` | `{llm_mode, stt_backend, tts_engine, xtts_language, paused, hud_show_text, plug_id_masked}` |
| PUT | `/config` | Whitelist only: `llm_mode` (`off`\|`fallback`\|`always`), `stt_backend` (`auto`\|`cuda`\|`npu`\|`cpu`), `tts_engine` (`auto`\|`xtts`\|`piper`\|`pyttsx3`), `xtts_language` (2–8 alpha), `paused` bool, `hud_show_text` bool. Unknown field or secrets: 400. |
| GET | `/workflows` | Registry JSON or `{workflows:{}}` |
| PUT | `/workflows` | Valid registry; webhook paths must not contain `://` |
| GET | `/contacts` | `{contacts: {name: masked}}` |
| PUT | `/contacts` | Merge; `""` deletes a name; digit length 8–15 |
| GET | `/timing` | `{rows:[{kind,stage,ms}]}` last 20 |
| POST | `/say` | `{"text"}` -> `{matched, reply}` via phrase router only |
| POST | `/thread/clear` | Clears Ollama thread (cap 20 turns in `brain.py`) |
| POST | `/dismiss` | Dismiss handler (epoch bump, stop audio, hide) |
| POST | `/shutdown` | Quit handler on a background thread |

### 8.3 WebSocket `/events`

~30 Hz frames:

```json
{
  "state": "listening",
  "label": "listening",
  "levels": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
  "paused": false,
  "you": "optional",
  "kuro": "optional"
}
```

`you` / `kuro` are included only when `hud_show_text` is on. Levels length 11. Shell client: `KuroShell/KuroShell/EngineClient.cs`.

---

## 9. Environment variables

### 9.1 Documented in `.env.example`

| Variable | Role |
| --- | --- |
| `KURO_LLM_MODE` | `off` \| `fallback` \| `always` (default `fallback`) |
| `OLLAMA_MODEL` | Chat model id |
| `KURO_STT_BACKEND` | `auto` \| `cuda` \| `npu` \| `cpu` |
| `KURO_WHISPER_MODEL` | e.g. `small.en` |
| `KURO_WHISPER_BEAM` | Beam size (partial STT uses 1) |
| `KURO_TTS_ENGINE` | `auto` \| `xtts` \| `piper` \| `pyttsx3` |
| `KURO_PIPER_MODEL` | Path to Piper ONNX |
| `TUYA_DEVICE_ID` / `TUYA_LOCAL_KEY` / `TUYA_IP` / `TUYA_VERSION` | Local plug |
| `N8N_INBOX_WEBHOOK_URL` / `N8N_WEBHOOK_SECRET` | Cloud inbox |
| `KURO_N8N_BASE_URL` / `KURO_N8N_TOKEN` | Self-hosted workflows |
| `KURO_HUB_URL` / `KURO_HUB_TOKEN` | Hub |

### 9.2 Also read by code

| Variable | Role |
| --- | --- |
| `KURO_HOTKEY` | Default `ctrl+alt+space` |
| `ONLY_WHEN_CHARGING` | Default false; boot currently treats charging gate as off |
| `OLLAMA_VISION_MODEL` | Default `qwen3-vl:8b` |
| `XTTS_SPEAKER_WAV` / `XTTS_LANGUAGE` | Voice clone + language |
| `KURO_WHISPER_COMPUTE` | e.g. `float16` |
| `KURO_OV_WHISPER_DIR` | Override OpenVINO model dir |
| `KURO_API` / `KURO_API_PORT` | Localhost API |
| `KURO_HUD_SHOW_TEXT` | Include transcripts on `/events` |
| `KURO_TIMING` | Write `%LOCALAPPDATA%\Kuro\timing.csv` |
| `KURO_HOME_SSID` | Wi-Fi watch home SSIDs (comma-separated) |
| `KURO_LEGACY_PYTHON` | Removed — Rust-only |

Never put secret values in the repo or this doc.

---

## 10. `%LOCALAPPDATA%\Kuro` state

| Path | Purpose |
| --- | --- |
| `api_token` | Localhost API shared secret |
| `kuro.log` | Rotating engine log |
| `timing.csv` | Stages only: command id, kind, stage, ms (no transcript) |
| `tuya_endpoint.json` | Cached plug IP / version |
| `n8n_workflows.json` | Voice workflow registry |
| `contacts.json` | WhatsApp name -> digits |
| `wa_session.db` | WhatsApp session |
| `google\token.json` | Google Meet OAuth token |
| `heartbeat.txt` | Liveness for watchers |
| `home_ssid.txt` | Home Wi-Fi names (one per line) |
| `DISABLED` | Autostart / Wi-Fi watcher refuse |
| `AWAY_OK` | Do not stop Kuro off home Wi-Fi |
| `wifi-watch.log` | Wi-Fi watcher log |
| `legion_mode.json` | Optional Legion power hint for compute policy |
| singleton lock dir | Single engine instance |

Also: conversation log used by the shell under the same tree as implemented by `ConversationLog.cs`.

---

## 11. STT / TTS / power policy

### 11.1 STT (`stt.py`, `compute_policy.py`)

- `auto`: CUDA `faster-whisper` `small.en` on AC; OpenVINO INT8 `bench/models/whisper-small.en-int8` on battery; CPU fallback.
- Forced backends: `cuda` / `npu` / `cpu`.
- Initial prompt biases desk phrases, n8n aliases, and contact names.
- Partial transcription uses beam 1; final uses `KURO_WHISPER_BEAM` (default 5).
- Production deps: `requirements.txt`. NPU / OpenVINO bench deps: `requirements-client.txt` (OpenVINO `2026.4.0`, `openvino-genai==2026.4.0.0`).

### 11.2 TTS

Python today (`tts.py`): `auto` is XTTS on AC and Piper on battery, with `pyttsx3` as the emergency voice. Fast path for phrase replies (`force_fast_tts`). `on_audio_start` flips the HUD to Speaking so Thinking holds until first audio. Repeated short lines use a clip cache.

Native acceptance for `kuro-engine.exe`:

- `/config` still accepts `tts_engine` `auto|xtts|piper|pyttsx3`.
- `auto` and `piper` run native ONNX (`KURO_PIPER_MODEL` through `ort` or `sherpa-onnx`) on AC and on battery, with sentence-boundary streaming and the same clip cache. No resident Python or PyTorch process.
- `pyttsx3` is Windows SAPI / WinRT `SpeechSynthesizer`.
- `xtts` is an on-demand external helper, or native Piper ONNX when that helper is absent.

### 11.3 Wi-Fi watch (`scripts/Watch-KuroWifi.ps1`, `docs/WIFI_WATCH.md`)

Polls / watches home SSID from `-HomeSsid`, `KURO_HOME_SSID`, or `home_ssid.txt`. Away from home can stop Kuro unless `AWAY_OK`. Native `WlanRegisterNotification` is a later replacement; behavior flags must stay equivalent.

---

## 12. Shell visual tokens (`DesignTokens.cs`)

| Token | Value |
| --- | --- |
| Face | Space Grotesk (Regular / Medium / Bold) |
| Mono | JetBrains Mono |
| Chat face | Poppins Regular / SemiBold |
| Chat radius | 22 |
| Canvas / Paper | `#F5F4EE` |
| Card / White | `#FFFFFF` |
| Ink | `#141414` |
| Muted | `#52514E` |
| Settings radius | 0 |
| Chat panel | `#0A0A0A` |
| Chat ink | `#F0F0F0` |
| Chat muted | `#A3A3A3` |
| Capsule height | 44 |
| Work area gap | 24 |
| Bar count / width / gap | 9 / 5 / 5 (defaults) |

Capsule position and size: set on initialization and display-change only. Do not call move/resize inside per-frame state or level updates.

---

## 13. Timing CSV

When `KURO_TIMING` is on, append rows to `%LOCALAPPDATA%\Kuro\timing.csv`:

- Columns: command id, kind, stage, milliseconds.
- Typical stages include `hotkey_down`, `hotkey_up`, `stt_done`, `route_done`, `idle`, plus one-shot kinds (`startup`, `plug`, TTS first-audio marks as implemented).
- Never store transcript text, contact names, or secrets.
- `GET /timing` returns the last 20 rows as `{kind, stage, ms}` only.

---

## 14. Parity test suite

### 14.1 Python (`pytest`)

- `tests/test_engine_api.py` — token, config whitelist, contacts/workflows, `/say`, dismiss, event frame text gating, thread clear
- `tests/test_n8n.py` — aliases, phrase routing, HTTP cases, registry parse
- `tests/test_whatsapp.py` — phrasings, contacts, clipboard vs desk power
- `tests/test_meet.py` — phrasings, missing login spoken, unknown contact
- `tests/test_timing.py` — CSV stages only; disabled writes nothing
- `tests/test_brain_thread.py` — follow-up memory and 20-turn cap / clear

### 14.2 Shell

```text
dotnet test .\KuroShell\KuroShell.Tests\KuroShell.Tests.csproj -c Debug -p:Platform=x64
```

Core spring / buffer / backoff tests in `KuroShell.Tests/CoreTests.cs`.

### 14.3 Manual checklist

Retain `docs/SHELL.md` manual steps: attach without double engine, PTT capsule Thinking-then-Speaking, phrase routes with Ollama off, pause listening, Leave it vs Stop engine.

---

## 15. Later phases (outline only)

Do not start these until this document is the working checklist.

1. **Phase 2 — Rust core:** workspace `kuro-rs/` with `kuro-hotkey` (`RegisterHotKey` + release poll), `kuro-router` (full phrase + hub + Tuya + Ollama clients), `kuro-wifi`, `kuro-ipc` (exact `127.0.0.1:8765` contract). `cargo test` mirrors the Python suite above.
2. **Phase 3 — Audio / STT / TTS:** WASAPI capture, unified Whisper ONNX / Sherpa with CUDA / OpenVINO / DirectML / CPU selection matching `compute_policy`, TTS backends with XTTS-compatible path retained until Kokoro acceptance, timing CSV parity.
3. **Phase 4 — Shell:** spawn `kuro-engine.exe` by default; source-generated JSON; no move/resize on HUD state ticks; `dotnet test` green.
4. **Phase 5 — Scripts / benchmarks:** `scripts/Build-KuroNative.ps1`, update `scripts/Start-Kuro.ps1`, compare idle RAM / VRAM / voice latency to the Python baseline in `docs/PERF_BASELINE.md` / `docs/PERF_RESULTS.md`.

Additive options after wire parity: named pipe IPC, headless Win32 tray + Direct2D mark when API is off, Kokoro as an extra TTS engine.

---

## 16. Acceptance for cutover

Native engine may become the default only when:

1. All routes in section 5 and tools in section 6 produce equivalent speakable outcomes.
2. Localhost API and WebSocket frames match section 8 bit-for-bit at the JSON field level (field names and semantics).
3. Hub and n8n clients honor section 7 and env defaults in section 9.
4. Dismiss / Escape cancel the in-flight turn (section 3.2).
5. Capsule does not move on state change; fonts and colors match section 12.
6. Parity suite (section 14) passes, and the shell manual checklist still holds.
7. Secrets remain outside git.

Until then, Python is the source of truth for behavior.

## 17. Native engine status

`kuro-engine.exe` is the only process for `scripts/Start-Kuro.ps1` and `KuroShell`. There is no Python fallback.

Speech weights:

- Whisper: in-process `whisper-rs` loads GGML `ggml-small.en.bin` from `KURO_WHISPER_GGML`, `%LOCALAPPDATA%\Kuro\models\`, or `bench/models/`. Beam 1 is used for partials and `KURO_WHISPER_BEAM` (default 5) for the final pass. The OpenVINO INT8 tree under `bench/models/whisper-small.en-int8` remains the documented NPU directory; this build does not link the OpenVINO C API.
- Piper: one long-lived `piper.exe` (under `%LOCALAPPDATA%\Kuro\piper` when installed) runs `bench/models/piper/en_US-lessac-medium.onnx` with `--output_raw`. Boot phrases are prebuilt into memory and `%LOCALAPPDATA%\Kuro\tts_cache`.
- `pyttsx3`: WinRT `Windows.Media.SpeechSynthesis.SpeechSynthesizer` returns PCM. There is no tone fallback.

IPC: HTTP stays on `127.0.0.1:8765`. Event frames also stream on `\\.\pipe\KuroEngine` with the same token. Quiet hidden/idle zero-level frames are skipped when unchanged.

Idle size and startup are in [PERF_RESULTS.md](PERF_RESULTS.md).

## 18. Adding New Phrases and Tools

Phrase order stays in `kuro-rs/crates/kuro-router/src/phrases.rs`. New spoken routes go there first when they are pure text matches.

For a new tool:

1. Implement `run(arguments: &Value) -> String` in a module under `kuro-rs/crates/kuro-tools/src/`.
2. Optionally implement `match_phrase(text: &str) -> Option<Value>` for spoken entry.
3. Add one `KuroTool` line inside `ToolRegistry::default_registry()` in `kuro-rs/crates/kuro-tools/src/lib.rs` (`name`, `description`, `parameters_schema`, `is_local_only`, `match_phrase`, `execute`).

`execute_tool` looks up that table. Meet, WhatsApp, and n8n phrase matching already go through the registry, so a new spoken tool is that type plus the registry line.

