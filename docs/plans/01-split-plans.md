# 01 Split plans for oversize files

Budgets (ADR 0002, file 02 section 6): target under 200 lines, hard cap 400 for
hand-written Rust, C#, PowerShell. Functions under about 60 lines. Generated data
files (`kokoro_vocab.rs`, `config.gen.rs`) are exempt by a `// @generated` first line.

Line counts are physical lines at commit `f661122`. Line ranges are the current
locations of the code to move. Every split is move-only unless the owning phase says
otherwise, and each keeps the public surface listed under "Stable surface".

## Rust

### `kuro-engine/src/main.rs` (621) - phases 12 and 13

```
kuro-engine/src/
  main.rs          under 60: call bootstrap::run, map errors to exit codes
  bootstrap.rs     singleton, local root, config load, log init (87 to 130)
  wiring.rs        build adapters, registry, router, turn driver (the Arc list)
  speech.rs        warm_speech, unload_speech
  hud.rs           HudSink adapter over the IPC state (set_hud)
  gate_mode.rs     KURO_GATE_CI branch
  threads/
    mod.rs         ThreadSupervisor: named threads and restart policy
    heartbeat.rs   (131 to 146)
    presence.rs    (147 to 166), event-based after phase 19
    api.rs         API thread, bind retry
    levels.rs      33 ms level publisher
    watchdog.rs    turn liveness
```

The `on_down`, `on_up`, `on_escape` closures and the follow-up loop (417 to 585) move
to `kuro-turn` in phase 12. `voice.rs` and `lib.rs` (`voice_turn`) are deleted then;
their tests move to `kuro-turn/tests/`.
Stable surface: binary name, env variables, exit behavior in gate mode.

### `kuro-router/src/phrases.rs` (702) - dissolved in phases 09 and 10

| Current content | Destination |
|-----------------|-------------|
| `normalize` and the corrections table (26 to about 110) | `kuro-router/src/normalize.rs`, generic entries in `kuro-router/data/corrections.toml`, owner entries in the profile `corrections.toml` |
| `matches`, `has_word` | `kuro-core/src/text.rs` (the one shared copy; the other three `normalize` functions call it) |
| `try_handle`, `execute_intent`, `legacy_try_handle` | removed; `Router::decide` plus the turn machine `Execute` effect |
| `is_*` predicates | each feature's `exact`, `exemplars`, or `parse` |
| reply strings, help text | each feature's `[replies]`; help is generated from enabled manifests |
| `power`, `speakable`, `news_to_speech` | deleted; typed `Outcome` and `FeatureError` |
| `fire_daddys_home`, `HOME_COOLDOWN_UNTIL` | `rituals` feature plus a ritual file in the owner profile |
| `route_command` | `kuro-turn` (LLM fallback decision) |
| tests | golden corpus and per-feature `tests.rs` |

Stable surface: `kuro_router::try_handle` and `normalize` stay as shims that ask the
new registry first and fall back to the legacy code, until the last feature has moved.

### `kuro-router/src/semantic.rs` (704) - phase 14, after phase 10

```
kuro-router/src/
  lib.rs          re-exports only
  normalize.rs
  exact.rs        tier-1 map built from manifests (158 to 200)
  similarity.rs   cosine, l2_normalize, dot (202 to 288)
  guards.rs       looks_like_question, conversational_help_suffix, polarity (301 to 341)
  decide.rs       TAU_INTENT, MARGIN_INTENT, MARGIN_LLM, decide_* (343 to 393)
  semantic.rs     route_semantic orchestration (395 to 410)
  pipeline.rs     stage order from data/pipeline.toml, RouteTrace
  embed/
    mod.rs        Embedder trait
    onnx.rs       session and state (412 to about 560), absorbs embed.rs, tokenizer.rs
    cache.rs      exemplar matrix cache keyed by manifest hash (about 560 to 652)
    hash.rs       hash_embed
  data/pipeline.toml
  data/corrections.toml
```

`Intent`, `EXEMPLARS`, `intent_tag`, `intent_from_tag` are deleted when the last
feature has moved. Until then a shim maps `Intent` to manifest ids.
Stable surface: the names imported by `tests/semantic_router.rs` keep working from
`kuro_router::semantic` until those tests are ported.

### `kuro-tts/src/kokoro.rs` (568) and `lib.rs` (385) - phase 14

```
kuro-tts/src/
  lib.rs            re-exports, Speaker facade
  speaker.rs        Speaker state and warm/unload
  cache.rs          clip cache (v3 files)
  sentences.rs      sentence split and prefetch
  playback.rs       play_pcm_live (moves behind AudioOut in phase 21)
  registry.rs       engine list and fallback chain (the registered file)
  engines/
    mod.rs          module declarations
    kokoro/
      mod.rs        synthesize (445 to 492), shutdown (494 to 498)
      paths.rs      (30 to 88)
      voice.rs      voice blending (94 to 169)
      phonemize.rs  espeak-ng (171 to 288)
      g2p.rs        simple_g2p (290 to 330)
      tokenize.rs   (332 to 346), uses kokoro_vocab.rs
      session.rs    ONNX slot (369 to 401)
      clause.rs     split_first_clause (407 to 443)
      tests.rs      (500 to 568)
    piper/mod.rs    from piper.rs
    winrt/mod.rs    from sapi.rs
```

The resampler (348 to 367) moves to `kuro-core/src/dsp/resample.rs`, the single copy
used by audio, STT, and TTS.
Stable surface: `Speaker`, `synthesize`, `piper_models_present`, `TtsEngine`.

### `kuro-tools/src/wa_neonize.rs` (410) and `whatsapp.rs` (397) - phase 10

```
kuro-features/src/features/whatsapp/
  feature.toml  README.md  tests.rs  routing_cases.toml
  mod.rs        Feature impl (whatsapp.rs 288 to 397)
  parse.rs      phrase regex and kinds (275 to 300)
  contacts.rs   contacts file and DB (82 to 191)
  resolve.rs    name resolution (193 to 273)
  source.rs     clipboard and browser URL
  bridge/
    mod.rs      Bridge trait, bounded worker (wa_neonize 248 to 353)
    ffi.rs      libloading bindings (21 to 61), cfg(windows)
    proto.rs    protobuf helpers (166 to 204)
    link.rs     link state for health; no file opening (206 to 246)
    deliver.rs  deliver_text (369 to 405)
```

Statics become worker fields. `Box::leak` is replaced by a context owned by the
worker. This split changes structure, so it is not move-only; the phase lists its
tests.

### `kuro-tools/src/meet.rs` (396) - phase 10

```
kuro-features/src/features/meet/
  feature.toml  README.md  tests.rs  routing_cases.toml
  mod.rs        run, run_native (202 to 265), RAII busy guard
  parse.rs      phrase match (52 to 148)
  spaces.rs     create-space call (374 to 396)
  browser.rs    Edge path (150 to 200), cfg(windows)
kuro-features/src/shared/google_auth/
  mod.rs        token load, refresh, scopes from manifests (267 to 372)
  expiry.rs     chrono-based expiry (285 to 298)
  tests.rs
```

### `kuro-ipc/src/lib.rs` (474) - phase 15

```
kuro-ipc/src/
  lib.rs      build router, serve
  state.rs    AppState, HudSnap
  auth.rs     token check (PROTECTED)
  frames.rs   event frames from kuro-protocol types
  pipe.rs     named pipe (cfg(windows))
  routes/     status.rs config.rs lists.rs timing.rs turn.rs shutdown.rs events.rs
```

### Others near the cap

| File | Lines | Plan |
|------|-------|------|
| `kuro-audio/src/lib.rs` | 316 | `capture.rs`, `device.rs`, `follow_up.rs`, `levels.rs`, `gain.rs` (phase 14) |
| `kuro-tools/src/n8n.rs` | 315 | `features/workflows/{mod,registry,client,parse}.rs` (phase 10) |
| `kuro-tools/tests/parity.rs` | 357 | split per feature as each migrates (phase 10) |
| `kuro-hotkey/src/lib.rs` | 292 | PROTECTED; under the hard cap; not split |
| `scripts/Start-Kuro.ps1` | 283 | shrinks under 200 in phase 18 |

## C# shell

All shell splits keep type names by using `partial class`, so callers do not change.

### `NativeCapsule.cs` (919) - phase 16

```
KuroShell/KuroShell/Capsule/
  CapsuleWindow.cs      class, create, WndProc, public API (66 to 198, 720 to 745)
  CapsuleState.cs       constants and fields (14 to 64)
  CapsuleRenderer.cs    Draw (200 to 272)
  CapsulePlacement.cs   Reposition (274 to 311); PROTECTED; only file allowed to position the window
  CapsuleCaption.cs     caption, shimmer, font (313 to 465)
  CapsulePixels.cs      primitives, Put, BlendPixel (467 to 553, 644 to 681)
  CapsuleBars.cs        level bars (555 to 642)
  CapsuleHitTest.cs     (683 to 718)
  Win32/CapsuleNative.cs  structs and P/Invoke (747 to 918)
```

### `HudWindow.cs` (602) - phase 16

```
KuroShell/KuroShell/Hud/
  HudWindow.cs         fields, ctor, ShowState (17 to 229)
  HudConversation.cs   (231 to 284)
  HudLevels.cs         SetLevels, Dismiss (286 to 306)
  HudDock.cs           toggle and RenderDock (308 to 370, 497 to 595)
  HudAnimations.cs     (372 to 428), uses KuroShell.Core Motion
  HudPlacement.cs      placement and tick (430 to 475)
  HudScroll.cs         (477 to 495)
```

### `MainWindow.xaml.cs` (506) and `MainWindow.xaml` (227) - phase 17

```
KuroShell/KuroShell/
  Shell/MainWindow.xaml       nav frame (81 to 115)
  Shell/MainWindow.xaml.cs    ctor, ShowPage through PageRegistry (10 to 71)
  Theme/Styles.xaml           styles (1 to 70)
  Pages/PageRegistry.cs
  Pages/PageHelpers.cs        (433 to 505)
  Pages/Conversation/         thread UI (392 to 431)
  Pages/Voice/                (170 to 263)
  Pages/Devices/              (265 to 296, devices part)
  Pages/Contacts/             (265 to 296, contacts part)
  Pages/System/               (298 to 352)
  Pages/HudPlayground/        (354 to 390)
```

`async void` handlers (lines 81, 94, 115) become `async Task` behind one helper that
reports errors.

### `EngineSupervisor.cs` (302) - phase 18

`Services/EngineSupervisor.cs` (process start and stop), `Services/EngineLocator.cs`
(candidate paths), `Services/TokenStore.cs`, and
`KuroShell.Core/Supervisor/RestartPolicy.cs` (pure, tested; absorbs
`RestartBackoff.cs`).

## Docs

### `docs/NATIVE_REWRITE_SPEC.md` (474) - phase 05

```
docs/spec/
  README.md            index; which files are normative, which are history
  01-process-map.md    section 2
  02-voice-turn.md     section 3
  03-routing.md        sections 4 and 5
  04-tools.md          section 6
  05-hub-wire.md       section 7
  06-engine-api.md     section 8
  07-config.md         section 9
  08-local-state.md    section 10
  09-stt-tts-power.md  section 11
  10-shell-visual.md   section 12
  11-timing.md         section 13
  12-history.md        sections 1, 14 to 17 (parity locks, cutover notes)
```

Section 18 becomes `docs/guides/add-a-feature.md`. The split fixes the known
contradictions: Python is removed, TTS values include `kokoro`, the HUD has 11 bars.
Later phases replace `04`, `06`, `07` with pointers to generated docs.
