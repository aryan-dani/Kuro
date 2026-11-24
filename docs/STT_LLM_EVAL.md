# STT and local LLM evaluation

Checkpoint A. No model has been chosen. No synthetic speech was generated. Session 3 is held out and must not be used for tuning.

## Machine

- GPU: NVIDIA GeForce RTX 5070 Laptop, 8151 MiB, driver 617.42.
- Ollama 0.33.2. Measured: `qwen2.5:7b-instruct`, `qwen3.5:4b`, `gemma4:e4b`.
- Not scored: `qwen3.5:9b` (about 9 GB, does not stay on this 8 GB GPU) and granite 4.2 (no confirmed tag used).
- Cargo 1.100.0.
- `nvcc` is not on PATH. CUDA Whisper and Parakeet CUDA builds are blocked until a CUDA 12.8 or newer toolkit is installed and `nvcc` works in a new terminal. Do not treat a missing toolkit as a benchmark result.

## What is in the tree

- `kuro-router` `classify_intent` is phrase-only. Eval must use it. Do not call `try_handle`, `route_command`, or music playback from a benchmark.
- `kuro-bench` binaries: `record`, `eval`, `mine`.
- `record` uses `kuro_audio::start_ptt_capture` (production device choice, 16 kHz mono) and writes WAV before `normalize_gain`.
- `eval` applies `normalize_gain(pcm, 0.15)`, then the production Whisper path (VAD trim and dedup) for `--engine whisper-small-cpu` only. Other engine names skip with a reason. Missing models skip. Tests do not need audio or a GPU.
- Mined command-shaped misses that the classifier rejected: 6 lines in `bench/data/voice/mined_errors.txt`. That file is gitignored. Do not commit it. Do not paste those lines into git.

The prompt catalog is at least 320 clips with one stand-in contact and one alias (unit test). The live contact book and workflow aliases add more. The recorder prints the exact count and an ETA. Plan on about 40 to 55 minutes of wall clock across three sessions if each clip is a short take plus the two Enter presses.

## How to record

Sit at the same desk. Plug in AC power. Set Windows to Performance mode (or the Lenovo performance preset). Use the same microphone Kuro already selects (the Realtek microphone array when it is present). Do not play the clips back as the test set. Read each line in your own voice.

From `kuro-rs`:

```
cargo run -p kuro-bench --bin record -- --session 1
cargo run -p kuro-bench --bin record -- --session 2
cargo run -p kuro-bench --bin record -- --session 3
```

- Session 1: quiet room, normal voice. Tag `quiet`.
- Session 2: realistic noise. Fan under load, or music or video at a normal volume. Tags `fan` and `music`.
- Session 3: hard set. Soft voice, faster speech, about 1 meter from the mic, tired if you can do it safely. Tags `soft`, `fast`, `far`, `tired`. This is the test set. Record it last, once.

Enter starts a clip. Enter stops it. Existing wav and label ids are never overwritten. Add `--resume` to continue a session after a break.

Clips land in `bench/data/voice/sessionN/` with `labels.jsonl`. That tree is gitignored. Do not commit wav files, per-clip transcripts, or `mined_errors.txt`.

Contact prompts are filled from `%LOCALAPPDATA%\Kuro\contacts.json` display names only. Numbers are not printed. Workflow lines come from aliases in `%LOCALAPPDATA%\Kuro\n8n_workflows.json`.

## Baseline, whisper small.en on CPU

Recorded clips: session 1 quiet 163, session 2 noise 117, session 3 held-out 69. Command plus tool clips on session 3: 48. Negatives: 20 per session (under 30 on the test set, so a gap under 1 point is not decisive). GPU was not used for this engine (`use gpu = 0`). The 935 MiB in the table is other GPU memory, not the Whisper model.

| engine | clips | command acc | false trigger | silence hallucination | mean WER | warm p50 ms | warm p95 ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| all | 349 | 0.812 (n=277) | 0.000 (n=60) | 0.727 (n=11) | 0.500 | 416 | 3028 |
| session 1 | 163 | 0.863 (n=139) | 0.000 (n=20) | 0.750 (n=4) | 0.496 | 380 | 3004 |
| session 2 | 117 | 0.789 (n=90) | 0.000 (n=20) | 0.714 (n=7) | 0.484 | 2024 | 3136 |
| session 3 | 69 | 0.708 (n=48) | 0.000 (n=20) | n/a (n=0) | 0.535 | 410 | 2783 |

Cold load was 3.6 s. False triggers on the negative lines were zero. Silence and noise clips still produced text on about 3 of 4 takes, so this baseline misses the "no hallucination on silence" gate. Warm p95 is about 2.8 to 3.1 s, above the 600 ms gate. p50 on the quiet and held-out sets is about 400 ms. Mean WER near 0.50 is mostly a trailing period (`Desk on` versus `Desk on.`), not a wrong command. Intent accuracy is the selection number.

## Whisper large-v3-turbo q5_0 on CUDA

Same 349 clips. Beam size 5. Full encoder context. Peak GPU memory during this run was 2155 MiB.

| engine | clips | command acc | false trigger | silence hallucination | mean WER | warm p50 ms | warm p95 ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| all | 349 | 0.874 (n=277) | 0.000 (n=60) | 0.727 (n=11) | 0.466 | 278 | 320 |
| session 1 | 163 | 0.892 (n=139) | 0.000 (n=20) | 0.750 (n=4) | 0.493 | 265 | 310 |
| session 2 | 117 | 0.867 (n=90) | 0.000 (n=20) | 0.714 (n=7) | 0.421 | 302 | 331 |
| session 3 | 69 | 0.833 (n=48) | 0.000 (n=20) | n/a (n=0) | 0.476 | 271 | 323 |

Held-out command accuracy is 83.3 percent, against 70.8 percent for small.en on CPU. False triggers stay at zero. Warm p95 is 323 ms, under the 600 ms gate. Silence and noise clips still produce text at the same 0.727 rate as the CPU baseline, so that gate is still missed. Parakeet was not run: the current ONNX stack would add a second runtime beside the pinned one. Among the two engines that were measured, turbo is the one to wire.

## Chat models

Context 4096, thinking off, temperature 0.2. Three runs of the 40 prompts in `bench/llm_quality.jsonl`. Warm numbers drop run 1. `qwen3.5:9b` was not scored: the 9 GB weight does not stay on an 8 GB GPU, and the download kept stalling. I (an LLM) scored quality. That favors fluent, cautious answers and can miss taste the user would prefer.

| model | GPU | warm first-token p95 | warm done p95 | replies ending in ? | over 220 chars | quality | format | latency | VRAM | weighted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| qwen2.5:7b-instruct | 100% (4.7 GB) | 38 ms | 642 ms | 2% | 0 | 4.0 | 5.0 | 4.0 | 3.5 | 83 |
| qwen3.5:4b | 100% (3.1 GB) | 130 ms | 794 ms | 10% | 13 | 3.5 | 3.0 | 3.0 | 5.0 | 69 |
| gemma4:e4b | 100% (nvidia-smi about 5.5 GB) | 71 ms | 423 ms | 5% | 0 | 4.5 | 4.5 | 5.0 | 2.5 | 88 |

Weights: quality 50, format 20, latency 20, VRAM headroom 10. All three passed 100 percent GPU and a first-token p95 under 1.5 seconds. Gemma refused fake device control, stayed short, and answered the Hinglish prompt in Hinglish. The 7B model once claimed it was shutting down. The 4B model explained Rust's question-mark operator incorrectly and asked extra questions. Gemma is the pick. With Gemma loaded, about 2.6 GB stayed free. Turbo weights are 547 MB, so the pair leaves more than 0.7 GB.

## After recording (baseline is in)

Baseline first, on the same clips, AC power, Performance mode, nothing else heavy:

```
cargo run -p kuro-bench --bin eval -- --engine whisper-small-cpu
```

Then CUDA turbo and Parakeet only after `nvcc` works and the models are on disk. Selection rules stay the ones in the plan: false triggers not worse than baseline, no text on silence and noise, warm p95 at or below 600 ms for a 3 second clip, at least 0.7 GB VRAM free with the chosen LLM loaded, highest session 3 command accuracy, lower VRAM on a tie under 1 point. If two leaders are inside that noise, or a class has fewer than 30 clips, stop and ask.

No fine-tune until session 3 accuracy is under 97 percent and there are at least 300 labeled clips, and only after an explicit yes.

Production STT and the Ollama default are unchanged in this checkpoint.
