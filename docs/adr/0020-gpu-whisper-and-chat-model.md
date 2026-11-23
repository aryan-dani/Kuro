# ADR 0020: GPU Whisper and the default chat model

- Status: Accepted
- Date: 2026-11-24
- Related: measured voice clips under bench/data/voice; docs/STT_LLM_EVAL.md

## Context

The shell forced `KURO_STT_BACKEND=cpu`, so the engine never used the GPU even when CUDA was installed. On this machine, Whisper small.en on CPU scored 70.8 percent command accuracy on the held-out session and a warm p95 near 2.8 seconds. Whisper large-v3-turbo q5_0 on CUDA scored 83.3 percent and a warm p95 of 323 ms, with the same zero false-trigger rate.

The chat default was `qwen2.5:7b-instruct`. Newer tags that fit in 8 GB are compared in the same eval note. A 9 GB weight does not stay fully on this GPU.

## Decision

The supervisor passes `cuda` when the CUDA 13 runtime directory is present, and puts that directory on the engine `PATH` so `cudart` loads. The engine looks for `ggml-large-v3-turbo-q5_0.bin` before `ggml-small.en.bin`. The chat call disables thinking and uses a 4096 token context. The default model id is the one that passed the GPU-resident gate in the eval note.

## Consequences

Machines without the CUDA runtime directory keep the previous CPU path. CI does not enable the `cuda` cargo feature. Silence clips can still produce text. That rate did not improve with turbo.
