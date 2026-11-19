# ADR 0011: LLM provider port

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 7; findings 10, N10; phases 19, 20, 26

## Context

`kuro-tools/src/brain.rs` calls Ollama directly: non-streaming, 60 second timeout,
model name from three inconsistent defaults. When Ollama is down the engine says it
is starting it and does nothing. The owner leans toward a provider trait with Ollama
as default and any OpenAI-compatible endpoint, with streaming to TTS as a product
item.

## Options

1. One adapter per vendor.
2. Two adapters: Ollama native, and OpenAI-compatible chat completions.
3. A third-party multi-provider client crate.

## Decision

Option 2, in a new crate `kuro-llm`.

- Port: `Llm::chat(request) -> stream of events` (`Token`, `ToolCall`, `Done`,
  `Error`). Non-streaming use collects the stream.
- Providers: `ollama` (default) and `openai_compatible` (covers LM Studio, llama.cpp
  server, vLLM, and hosted APIs). `none` disables the LLM cleanly.
- Local by default. A non-loopback base URL requires `privacy.allow_cloud = true`,
  and `kuro doctor` prints where prompts are sent.
- Reachability is checked per turn with a short cached probe, never once per
  conversation.
- "Down" behavior is a setting: `report` (default; "The language model is off.") or
  `start` (spawn `ollama serve`, wait up to 10 seconds). The sentence always matches
  what actually happens.
- One default model name, declared once in the `llm` settings section and chosen by
  the user in `kuro init`.
- The conversation thread and system prompt move from `brain.rs` to `kuro-turn`
  (context) and the profile (persona).

A multi-provider crate is rejected: it adds a large dependency tree for two HTTP
shapes that `ureq` already handles.

## Consequences

- Streaming to TTS and tool calling need no further port changes.
- Cloud use is possible but never accidental.
- Vendor-specific features (for example vision input) go through capability flags on
  the provider.

## Enforced by

`llm::tests` with scripted HTTP fixtures; `llm::tests::down_message_matches_policy`;
the config validation that rejects a remote URL without `privacy.allow_cloud`.
