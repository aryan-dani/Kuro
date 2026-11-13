# Performance results

Before numbers are the 20-command run in [PERF_BASELINE.md](PERF_BASELINE.md). After numbers are the stages Phase 1 actually changed. The shell does not sit on the voice path, so those stage times still stand after the WinUI work.

The desk plug was read with `status()` only.

## Kept

XTTS time to first audio, one long reply of four sentences.

- Before, news `tts_first_audio` p50 3468.0 ms (the whole reply was synthesized first).
- After, sentence playback: 374.5 ms.
- That is well past a 10 percent or 50 ms cut. `inference_stream` did not run on this torch build (the stream generator raises inside `isin`), so that path was not kept. Sentences use the existing `inference()` call, and the next sentence is synthesized while the current one plays.

Short repeated line, "Done, sir."

- First synthesis `tts_first_audio` 329.3 ms, in line with the music baseline p50 of 330.8 ms.
- Second call, after the 2.5 s duplicate guard, `tts_first_audio` 0.7 ms from the in-memory clip cache.

Speech to text, beam size 1 on clips of 12 seconds or less. Twelve warm samples, p50 234.5 ms.

- Before, music p50 320.5 ms and news p50 427.6 ms on the same short clip.
- The drop is more than 50 ms. Beam size 1 stays.

Plug `status()` with one persistent socket. Twelve samples across three processes: 369.4, 30.0, 43.2, 32.2, 200.5, 27.5, 29.6, 32.4, 357.7, 29.2, 48.5, 33.5.

- Before, once the device was known, each call was about 450 ms (449.9, 454.0, 484.9). The first discovery in that run was 160982.2 ms.
- After, the first call in a process is still a connect (about 200 to 370 ms). Later calls in that process are about 30 ms. That is more than a 50 ms cut, so the persistent socket stays.

## Not changed

Ollama on a phrase miss stays as it was (llm `route_done` p50 1786.5 ms, p95 14220.3 ms). No code change in that list was a fit.

Cold XTTS load stays serial with Whisper. Loading both at once has faulted CUDA before, and the warm is not on the per-command path once the models are up. Baseline `xtts_load` was 12753.4 ms once.

Long-reply playback (`tts_done`) is still about the length of the audio. The win on that path is hearing the first sentence early, not a shorter reply.

## Native engine, 11 Nov 2026

Release `kuro-engine.exe` from `kuro-rs/target/release`.

- File size: 9,019,904 bytes (8.6 MB).
- Time from process start until the working set passed 5 MB: 736 ms.
- Idle working set one second later, before the Whisper worker: 20.9 MB. Private bytes: 2.2 MB.
- With the worker warmed (`KURO_STT_PREWARM=1` on this run, because the laptop was not on the home SSID): engine working set 13.5 MB and worker working set 11.7 MB. `POST /shutdown` stopped the worker.
- Whisper small.en, CPU int8, beam 5, on a spoken "hello":
  - Weight load: 1334 ms.
  - First transcription: 1640 ms, text `Hello.`
  - Second transcription, model already loaded: 1765 ms, text `Hello.`
- The earlier 8.3 s figure was the whole test, including Windows speech synthesis, process start, and the first load. Pre-warm moves that load off the first hotkey. Away from the home SSID, the engine does not keep the worker running.
- The Python engine that was already running (`run_kuro.py`) had a working set of 40 MB. Its private commit was about 4.6 GB because the PyTorch and Whisper runtimes stay mapped. The native process does not load those libraries at idle.

Whisper transcription uses the local faster-whisper small.en weights through one persistent worker. Piper speech uses `piper.exe` and `bench/models/piper/en_US-lessac-medium.onnx`. `pyttsx3` uses WinRT `SpeechSynthesizer`. A spoken "hello" WAV came back from Whisper as a real transcript in the `kuro-stt` test (8.3 s, including model load).

## Pre-native-tools (12 Nov 2026)

Python spawn / import tax measured from the repo venv before the port. Spoken paths still used the Python sidecars at this point.

| Path | ms | Spoken / note |
| --- | ---: | --- |
| Cold `tools.meet` import | 903 | interpreter + google auth imports |
| Cold `tools.wa_link` import | 590 | neonize import path |
| Cold `tinytuya` import | 1242 | plug fallback tax |
| Chat `who are you` via `/say` | 8 | `Kuro. Local assistant. Phrases first, model last.` |
| Plug via `/say` (engine missing TUYA env) | 225 | `Smart plug credentials are missing in .env.` |
| Meet via `/say` while Python sidecar ran | blocked API | hung the current-thread HTTP loop on neonize/Edge |

The 10x bar for this port is that cold Python tax, not Google/Edge/WhatsApp wall clock.

## Post-native-tools (12 Nov 2026)

In-process Meet, WhatsApp (neonize DLL), and Tuya framing. No `python.exe` tool children.

| Path | ms | Spoken / note |
| --- | ---: | --- |
| Chat `who are you` | 4 | same spoken line |
| Desk on | 2005 | `Desk on.` (native TCP; credentials loaded from `.env`) |
| Desk off | 2012 | `Desk off.` |
| `Send the Google Meet link to Mom` | 4826 | `Sent the Meet link to Mom.` |
| Immediate second Meet | 208 | `I'm already opening a Meet.` (busy + 45s cooldown) |
| `python.exe` tool children during Meet | 0 | confirmed via process list |

Python import tax removed from the tool path (903 / 590 / 1242 ms cold imports no longer run). Meet wall clock stays network-bound (~5 s here) and is no longer a new interpreter per call.

## Semantic router, Kokoro, hot path (12 Nov 2026)

Scaffold landed; models were still optional at that point.

## Live ONNX close-out (12 Nov 2026)

Models on disk under `bench/models/{router-embed,kokoro,silero-vad}` (mirrored to `%LOCALAPPDATA%\Kuro\models\…`). Direct `ort` 2.0.0-rc.15 (shared `Environment` + CPU arena). No `kokoro-en`.

| Metric | Value | Note |
| --- | --- | --- |
| Router ONNX forward (bge-small WordPiece + mean-pool) | **12.103 ms** | release, warm session, 30 iters avg |
| AVX2/FMA hand-written 384-d dot | **0.62 ns** (0.0006 µs) | release, 500k iters; `cosine_similarity_384` |
| Kokoro-82M `af_heart` first-audio (uncached) | **1314.77 ms** | release, direct ort + G2P, "Desk on." |
| Kokoro second uncached sentence | **301.51 ms** | same session, "Alright." |
| Ritual clip cache path | memory/disk hit | `Speaker` + `tts_cache` after `prebuild_phrase_cache` / prior synth |
| Warm STT (whisper-rs, spoken WAV, beam=1) | **17622.6 ms** | release test on this host (pcm≈1.3 s @22.05 kHz); Silero optional via `KURO_DISABLE_SILERO`; prior engine path was ~1765 ms |
| Peak WS with Whisper+Kokoro+router mapped | **705.7 MB** | unload integration test |
| WS after dormant unload + `EmptyWorkingSet` | **0.2 MB** | `< 15.0 MB` required; pages fault back on next use |
| Idle engine WS (API, no warm) | ~14 MB | gate smoke style |
| Python layout | removed | runtime is Rust-only; optional `scripts/bench/*.py` only |

Download helpers: `scripts/Download-RouterEmbed.ps1`, `Download-KokoroModel.ps1`, `Download-SileroVad.ps1`. Measure: `scripts/Measure-KuroOnnx.ps1`.


## Surgical latency fixes (12 Nov 2026)

Fixes applied: `kuro-stt` reuses `WhisperState`, scales `audio_ctx` dynamically to the trimmed clip, uses greedy decoding with `temperature_inc=0.0`, and compiles `ggml` with MSVC `/O2 /arch:AVX2 /fp:fast`; `kuro-router` uses dynamic sequence length `[1, seq_len]` with a 4-token init warm-up and 4 intra-op threads; `kuro-tts` warms up the Kokoro `ort` session on init, caches voice style rows and `espeak` lookup, and splits opening clauses > 40 chars when streaming.

| Metric | Before | After | Note |
| --- | ---: | ---: | --- |
| Router ONNX forward (dynamic `seq_len`, warm session) | 12.103 ms | **2.371 ms** | release, 30 iters avg (`onnx_bench`) |
| AVX2/FMA 384-d dot | 0.62 ns | **0.53 ns** | release, 500k iters (`0.0005 us`) |
| Kokoro-82M `af_heart` first-audio (after init warm-up) | 1314.77 ms | **200.39 ms** | release, "Desk on."; init+warm-up 875.63 ms once |
| Kokoro second uncached sentence | 301.51 ms | **198.09 ms** | same session, "Alright." |
| Ritual clip cache path | memory/disk hit | **memory/disk hit** | `Speaker` + `tts_cache` (0 synth calls on repeat) |
| Warm STT (`whisper-rs`, AVX2 `ggml`, reused state, dynamic `audio_ctx`) | 17622.6 ms | **349.1 ms** | release (`cold_stt_ms=954.5`), spoken "hello" WAV |
| Peak mapped WS -> after dormant unload | 705.7 -> 0.2 MB | **792.0 -> 0.2 MB** | `unload_working_set` release (`< 15.0 MB` required) |
