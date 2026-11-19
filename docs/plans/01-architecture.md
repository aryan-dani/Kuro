# 01 Target architecture

Decisions here are recorded as ADRs in [../adr/](../adr/README.md). File split plans
are in [01-split-plans.md](01-split-plans.md).

## 1. Target tree (depth 3)

```
Kuro/
  AGENTS.md                 agent entry point: rules, commands, map pointers
  ARCHITECTURE.md           one page: layers, data flow, where things live
  LICENSE                   MIT
  THIRD_PARTY_NOTICES.md    models, fonts, native tools
  models.lock.toml          model name, url, sha256, license, used by
  .cursor/rules/            scoped agent rules
  .github/                  workflows, CODEOWNERS, templates, dependabot
  contract/
    v1/                     golden JSON fixtures for the engine-shell protocol
  profiles/
    example/                harmless tracked profile
  docs/
    adr/                    decisions
    plans/                  this plan
    spec/                   split of NATIVE_REWRITE_SPEC.md
    guides/                 add a feature, add a setting, add a page, release
    features/               generated, one page per feature
    REPO_MAP.md             generated
    CONFIG.md               generated
  kuro-rs/
    xtask/                  gate, checks, scaffolders, generators, bench
    fixtures/
      phrases/              golden routing corpus
      audio/                synthetic WAV clips for STT replay
    crates/
      kuro-core/            pure domain: ids, errors, epoch, text, ports, feature API
      kuro-config/          settings loading, validation, profile, secrets
      kuro-protocol/        wire types, version, fixture tests
      kuro-router/          normalize, exact, semantic, decide (never executes)
      kuro-features/        one folder per feature plus the registry
      kuro-llm/             LLM providers
      kuro-turn/            turn state machine, follow-up policy, context
      kuro-ipc/             HTTP, WebSocket, named pipe
      kuro-audio/           capture and playback adapters
      kuro-stt/             STT backends
      kuro-tts/             TTS engines
      kuro-triggers/        activation sources: hotkey wrapper, wake word
      kuro-hotkey/          PROTECTED low-level keyboard hook
      kuro-wifi/            presence adapter
      kuro-engine/          composition root and thread supervision only
      kuro-cli/             `kuro` binary: init, doctor, route, say
  KuroShell/
    KuroShell/
      Capsule/              native layered-window capsule
      Hud/                  transcript card and dock
      Pages/                one folder per page plus PageRegistry.cs
      Services/             engine client, supervisor, tray, escape hook
      Theme/                tokens and styles
      Shell/                main window frame and chrome
    KuroShell.Core/
      Protocol/             DTOs checked against contract/v1
      Supervisor/           restart policy (pure)
      Motion/               spring and level smoothing (pure)
      Conversation/         transcript buffer
      Settings/             schema-driven settings model
    KuroShell.Tests/
  scripts/                  thin wrappers over cargo xtask, install helpers
  examples/
    n8n/                    sanitized workflows and compose file
```

New crates: `kuro-config`, `kuro-protocol`, `kuro-features` (replaces `kuro-tools`),
`kuro-llm`, `kuro-turn`, `kuro-triggers`, `kuro-cli`, `xtask`. `kuro-tools` and
`kuro-router/src/phrases.rs` are deleted when their last content has moved.

## 2. Crate layering (ADR 0002)

| Crate | May depend on | In the Linux job |
|-------|---------------|------------------|
| kuro-core | nothing in the workspace; no `windows*`, `ort`, `cpal`, `rodio` | yes |
| kuro-config | core | yes |
| kuro-protocol | core | yes |
| kuro-router | core, config | yes (ONNX behind the `onnx` feature; tests use the hash embed) |
| kuro-llm | core, config | yes |
| kuro-features | core, config | yes for logic; Windows code only in files named `win.rs` behind `cfg(windows)` |
| kuro-turn | core, config | yes |
| kuro-ipc | core, config, protocol | HTTP and WS yes; pipe is `cfg(windows)` |
| kuro-audio, kuro-stt, kuro-tts, kuro-wifi, kuro-hotkey | core, config | no |
| kuro-triggers | core, config, hotkey | no |
| kuro-engine | everything | no |
| kuro-cli | core, config, protocol, router, features | no |
| xtask | no product crate | yes |

Rules: adapters never depend on each other. Nothing depends on `kuro-engine`. The
router never executes a feature. Only `kuro-config` reads environment variables. Only
the HTTP adapter calls `ureq`. Enforcement: `cargo xtask check arch` reads
`cargo metadata` and fails the gate (file 02, section 5).

Today three edges violate this and must be cut: `kuro-core -> ort, windows-sys`,
`kuro-router -> kuro-tools`, `kuro-ipc -> router, tools, tts`.

## 3. Extension model (ADR 0003, 0004, 0006)

A feature is one folder under `kuro-rs/crates/kuro-features/src/features/<id>/`:

| File | Purpose | Cap |
|------|---------|-----|
| `README.md` | what it does, intents, settings, failure modes | 60 lines |
| `feature.toml` | manifest: intents, phrases, exemplars, replies, settings, tier | data |
| `mod.rs` | `Feature` impl: `parse`, `execute`, `health`, `on_event` | 200 lines |
| `config.gen.rs` | typed settings struct, generated from `feature.toml` | generated |
| `tests.rs` | unit and failure-injection tests with fakes | 400 lines |
| `routing_cases.toml` | golden routing cases owned by this feature | data |
| other `*.rs` | one concept each (client, parse, speak) | 200 lines |

Manifest shape:

```toml
id = "timers"
title = "Timers and reminders"
tier = "reversible"            # read_only | reversible | external_send | irreversible
default_enabled = true
priority = 50                  # pattern-stage order; duplicates fail at startup
requires = ["scheduler", "announcer", "store"]
stt_bias = ["timer", "remind me"]

[[intents]]
id = "timers.set"
exact = ["set a timer"]
exemplars = ["set a timer for ten minutes", "start a five minute timer"]
slots = ["duration"]
llm_description = "Start a countdown timer."

[replies]
set_ok = "Timer set for {duration}."
not_configured = "Timers are not available."

[[settings]]
key = "max_active"
type = "int"
default = 10
min = 1
max = 50
doc = "Maximum number of timers running at once."
```

Trait (lives in `kuro-core/src/feature/`, so `kuro-turn` needs only `kuro-core`):

```rust
pub trait Feature: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn parse(&self, _u: &Utterance) -> Option<Invocation> { None }
    fn execute(&self, inv: &Invocation, ctx: &Ctx) -> Result<Outcome, FeatureError>;
    fn health(&self, _ctx: &Ctx) -> Health { Health::ok() }
    fn on_event(&self, _ev: &KuroEvent, _ctx: &Ctx) -> Option<Announcement> { None }
}
pub struct Outcome { pub speech: Spoken, pub follow_up: FollowUp, pub data: Option<Value> }
pub enum FeatureError { NotConfigured, Unreachable, Timeout, Unauthorized, Rejected,
    InvalidInput, Busy, DeviceUnavailable, Disabled, Internal }   // each carries detail
```

`Ctx` gives a feature everything it may touch: `config`, `secrets`, `http` (timeouts
mandatory), `clock`, `store`, `scheduler`, `announcer`, `log`, `turn_id`, `epoch`.
Features hold no statics and spawn no threads; long-lived work goes through
`ctx.worker(name, fn)` so the engine supervises it.

Registration is one checked file, `kuro-features/src/features/mod.rs`:

```rust
// xtask:features:begin
pub mod desk_power;
pub mod timers;
// xtask:features:end
pub fn all() -> Vec<Box<dyn Feature>> {
    vec![
        // xtask:registry:begin
        Box::new(desk_power::DeskPower::default()),
        Box::new(timers::Timers::default()),
        // xtask:registry:end
    ]
}
```

An architecture test fails if a folder is not registered or a registered name has no
folder. No `inventory`, no `build.rs` discovery.

Dynamic sources: features discovered at run time (n8n workflows from
`workflows.json`, hub tools, later MCP servers) implement `DynamicSource` and return
manifests on demand. They match by exact name and alias only.

Files touched to add a feature: the new folder, two scaffolder-inserted lines in
`features/mod.rs`, and generated docs. Nothing in the engine, router, turn, IPC, or
shell.

Routing pipeline (order identical to today, now explicit in
`kuro-router/data/pipeline.toml`): pending slot owner, exact phrases, semantic match
with guards, pattern matchers by `priority`, LLM. The router returns a decision plus a
trace that `kuro route --explain "<text>"` prints.

Rituals (data, ADR 0004): `profiles/<name>/rituals/*.toml` hold a phrase list, a
cooldown, a reply, and a linear list of feature calls with an optional `parallel`
flag. No conditions, no loops. The built-in `rituals` feature runs them.

## 4. Ports and adapters

Traits live in `kuro-core/src/ports/`, one file each.

| Port | Adapter today | Template scope | Later |
|------|---------------|----------------|-------|
| Trigger | hotkey | yes | wake word (phase 25) |
| Mic | cpal | yes | |
| AudioOut | rodio | yes | |
| Stt | whisper-rs CPU | yes | GPU and NPU backends |
| Tts | Kokoro, Piper, WinRT | yes; WinRT is the zero-download default | |
| Llm | Ollama | yes, plus OpenAI-compatible | |
| Router | kuro-router | yes | |
| HudSink | IPC frames | yes | |
| Clock | system | yes | |
| HttpClient | ureq | yes | |
| SecretStore | file in local root | yes | DPAPI |
| Presence | kuro-wifi | yes, mode `always` by default | |
| Clipboard, ScreenCapture | Win32 | yes | |
| Store (memory store) | none | phase 22, SQLite via existing `rusqlite` | |
| Scheduler | none | phase 22 | |
| Announcer (notifier) | none | phase 22 | Windows toast through the shell |
| EventBus | none | phase 22, bounded in-process fan-out | |
| Device | Tuya local | example feature, off by default | Home Assistant |
| HubTransport | HTTP and WS, unwired | optional feature, off by default | |

## 5. Turn state machine (ADR 0008)

A pure function `step(state, event) -> (state, effects)` in `kuro-turn`. One driver
thread executes effects through ports and feeds results back as events.

| State | Event | Next | Effects |
|-------|-------|------|---------|
| Idle | Activate(hold) | Listening | bump epoch, StartCapture, Hud(listening) |
| Idle | Activate(hands_free) | Listening | bump epoch, StartVadListen, Hud(listening) |
| Idle | Announce | Speaking | Speak, Hud(speaking) |
| Listening | Release or VadEnd | Transcribing | StopCapture, Transcribe, Hud(thinking) |
| Listening | Captured(too short) | Idle | Hud(idle) |
| Transcribing | Transcribed(text) | Routing | Route |
| Transcribing | SttFailed | Idle | Hud(error then idle), Log |
| Routing | Routed(feature) | Executing | Execute |
| Routing | Routed(llm) | Thinking | AskLlm |
| Routing | Routed(none) | Idle | Hud(idle) |
| Executing | Executed(outcome) | Speaking | Speak, Hud(speaking), Timing |
| Thinking | LlmSentence | Speaking | Speak (queue) |
| Thinking | LlmFailed | Speaking | Speak(error reply) |
| Speaking | SpeechDone | FollowUp or Idle | by policy: StartVadListen or Hud(idle) |
| Speaking | Activate | Listening | StopSpeech, bump epoch (barge-in) |
| FollowUp | VadEnd | Transcribing | Transcribe |
| FollowUp | Timeout | Idle | Hud(idle) |
| any | Cancel(escape, dismiss, api) | Idle | bump epoch, StopCapture, StopSpeech, Hud(idle) |
| any | PresenceChanged(away) | same | UnloadModels only when Idle, else defer |

Cancellation epoch: `TurnClock` keeps its current meaning (`begin_turn`,
`force_unblock`, `live(epoch)`). Every effect result carries the epoch it started
under; the machine drops stale results. Cancel has one entry point, so the engine
`busy` flag disappears and the three Escape paths collapse into one event.

Follow-up policy is data: `conversation.follow_up` is `always`, `question`, or
`never`; `follow_up_window_ms` 6000; `silence_ms` 850. The default stays `always`
through phase 19 so behavior is unchanged. A feature can also request a slot with
`FollowUp::Expect("duration")`; the answer is routed back to that feature.

Headless tests: fakes in `kuro-core/src/testkit/` (FakeMic, FakeStt, FakeTts,
FakeLlm, FakeClock, RecordingHud). A scenario reads like
`press, say("desk on"), release, expect_spoken("Desk on.")` and runs in milliseconds
with no audio device, model, or network.

## 6. Config, profile, secrets (ADR 0005)

Precedence, highest first:
1. Process environment (`KURO_*` and legacy names). Dev and CI only. A key pinned by
   env is reported as `locked` and is read-only in the shell.
2. `%LOCALAPPDATA%\Kuro\config.toml`: user settings written by `kuro init`, the shell,
   and `PUT /config`. API changes are persisted here.
3. Profile defaults: `<profile>/profile.toml`.
4. Built-in defaults declared in manifests and `kuro-config/sections/*.toml`.

Secrets live only in `%LOCALAPPDATA%\Kuro\secrets.toml` or env, are never returned by
`/config`, and are never logged. A repo-root `.env` still loads for developers, with
every schema key allowed (fixes bug 9).

Settings are declared as data (`[[settings]]` blocks). `cargo xtask gen` writes
`config.gen.rs` per section, `docs/CONFIG.md`, `contract/v1/config.schema.json`, and
`.env.example`. Validation (type, range, enum, unknown key) runs once at load; a bad
file exits with code 64 and a readable message.

Profile folder (`profiles/example/` tracked; the user's profile at
`%LOCALAPPDATA%\Kuro\profile\` or `KURO_PROFILE_DIR`):
`profile.toml` (assistant name, honorific, voice), `phrases.toml` (extra phrases per
intent), `corrections.toml` (mishearing fixes), `rituals/*.toml`, `contacts.json`,
`workflows.json`, `tests/routing.toml` (profile-owned corpus).

## 7. Protocol contract (ADR 0009)

`kuro-protocol` owns every wire type. `contract/v1/*.json` are golden fixtures. Rust
tests serialize canonical values and compare bytes; C# tests deserialize the same
files with unknown members disallowed. `/status` carries `protocol: 1`; the pipe and
WS send `HELLO {"protocol":1,"engine":"x.y.z"}` first. The shell refuses a different
major version with a visible message. Frames gain a `type` field (`hud`, `turn`,
`health`, `config`, `announcement`); a missing `type` means `hud`.

## 8. Supervision tree (ADR 0010)

```
Task Scheduler logon task (restart on failure)          unverified for WinUI apps
  KuroShell.exe  (single-instance mutex)
    EngineSupervisor  (the only engine restart authority)
      kuro-engine.exe  (singleton mutex)
        main thread     hotkey message loop        failure is fatal (exit 70)
        turn thread     state machine driver       restart 3 times, then exit 70
        api thread      HTTP, WS, pipe             bind retry every 5 s, never fatal
        heartbeat       15 s                       restart
        presence        WLAN events                restart; failure means "home"
        scheduler       timers and jobs            restart
        feature workers music player, WhatsApp     restarted lazily on next use
        watchdog        turn thread liveness       exits 70 when stalled 30 s
```

Engine exit codes: 0 clean (no restart), 64 invalid config (no restart, show the
message), 70 internal fatal (restart with backoff 1, 2, 5, 15, 60 s; give up after 5
restarts in 10 minutes). Headless mode: the logon task starts the engine directly.
`Watch-Kuro.ps1`, the `App` 15 s watchdog, and `Watch-KuroWifi.ps1` are removed.

Panic strategy (ADR 0007): `panic = "unwind"`, with `catch_unwind` at two
boundaries only (feature execute, effect execution on the turn thread), a panic hook
that logs with `turn_id`, and RAII guards for every busy flag.

## 9. Shell decomposition

The shell keeps three responsibilities: render (capsule, HUD, pages), supervise the
engine, and edit settings. All logic that can be tested without a window moves to
`KuroShell.Core`. Settings pages render from the config schema, so a new setting needs
no C# edit. Pages register in `Pages/PageRegistry.cs`. The capsule position code is
isolated in one protected file so "never moves on state change" is checkable.
Details and line-level moves: [01-split-plans.md](01-split-plans.md).

## 10. Preserved invariants

- Ctrl+Alt+Space hold-to-talk is owned by the engine (`kuro-hotkey`).
- The capsule is non-activating, bottom-anchored, and never moves on state change.
- The localhost API on 127.0.0.1 with `X-Kuro-Token` stays.
- Dismiss and Escape cancel through the turn epoch.
- Tool results reach the user as speakable strings.
- Secrets exist only in ignored files or `%LOCALAPPDATA%\Kuro`.
