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

