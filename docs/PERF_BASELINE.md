# Performance baseline

Measured on 5 Nov 2026 with `KURO_TIMING=1` and `scripts/bench/run_timing_baseline.py`.

Whisper loaded on CUDA (1604.6 ms). XTTS warmed on CUDA (12753.4 ms) before the 20 commands. The desk plug was read with `status()` only. It was not switched.

This run is harness-driven. It does not hold Ctrl+Alt+Space, so `hotkey_down`, `recording_start`, `hotkey_up`, and `audio_saved` are a few tenths of a millisecond and are not a live microphone hold. Every voice row transcribes the same short clip, then runs the labeled command. Three music replies were skipped by the existing 2.5 second duplicate guard, so those rows have no TTS samples (n=3, not 6).

p50 and p95 use the nearest rank on the sorted samples.

## Stage times (ms)

Voice commands (music n=6, news n=6, llm n=4):

- hotkey_down: p50 0.0, p95 0.0 (harness, not a key hold)
- recording_start: p50 0.3 to 0.4, p95 under 1
- hotkey_up: p50 0.3, p95 under 4
- audio_saved: p50 0.3 to 0.4, p95 under 1
- stt_done: music p50 320.5 p95 878.1; news p50 427.6 p95 478.0; llm p50 358.2 p95 382.5
- route_done: music p50 0.6 p95 217.3; news p50 76.2 p95 100.3; llm p50 1786.5 p95 14220.3
- tts_first_audio: music p50 330.8 p95 845.7 (n=3); news p50 3468.0 p95 5275.1; llm p50 1288.7 p95 1484.3
- tts_done: music p50 1143.4 p95 2868.3 (n=3); news p50 11160.5 p95 13087.7; llm p50 3824.7 p95 4191.8
- idle: p50 under 1

Startup, one sample each:

- whisper_load: 1604.6
- xtts_load: 12753.4

Plug `status()` four calls, in order: 160982.2, 449.9, 454.0, 484.9. p50 454.0. p95 160982.2. The first call discovered the device. The next three stayed near 450 ms.

## Top 5 contributors

1. XTTS time until the first sample (`tts_first_audio`). News waits about 3.5 s before any sound because the whole reply is synthesized first.
2. XTTS playback (`tts_done`). A news reply then plays for about 11 s. That is length, not startup, but it blocks the return to idle.
3. Ollama on a phrase miss (`route_done` for llm). p50 about 1.8 s. The cold call pulls p95 to about 14 s.
4. Cold XTTS load, 12.8 s, once per warm.
5. Speech to text, about 0.35 to 0.43 s p50 on this short clip. Plug status is next: about 0.45 s once the device is known, and 161 s when it has to be discovered.

The phrase router itself is not in this list. Music `route_done` p50 is 0.6 ms. News `route_done` is the feed fetch, about 76 ms.

## What Phase 1 will change

- Stream XTTS chunks so the first sample is not held for the full synthesis.
- Keep a short in-memory clip for repeated replies of 80 characters or fewer.
- Use beam size 1 for clips of 12 seconds or less, and drop it if the saving is under 50 ms and under 10 percent.
- Reuse one persistent plug socket after discovery. Do not switch the relay while measuring.

GPU Whisper and XTTS stay serial. Loading them together has already faulted CUDA, and that load is not on the per-command path once the models are warm.
