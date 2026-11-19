# ADR 0003: Feature extension model with explicit registration

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 2; findings S1, S2, N25; phases 09, 10, 11

## Context

Adding one tool touches 9 to 11 places in 4 crates (finding S1). The owner wants a
feature to be one folder with zero engine or router edits, self-describing, with
greppable registration and no magic.

## Options

1. Link-time auto-registration (`inventory`, `linkme`). Zero registration lines, but
   invisible wiring, linker-dependent behavior, and nothing to grep.
2. `build.rs` that scans folders and generates the list. Greppable only in
   `OUT_DIR`; hides errors from agents.
3. A checked-in registry file edited by a scaffolder, verified by an architecture
   test.
4. One crate per feature registered in the workspace. See ADR 0002.

## Decision

Option 3. A feature is a folder `kuro-features/src/features/<id>/` with a
`feature.toml` manifest and a type implementing `kuro_core::feature::Feature`
(`manifest`, `parse`, `execute`, `health`, `on_event`). The manifest carries intents,
exact phrases, exemplars, slots, replies, settings, permission tier, STT bias terms,
required capabilities, and the LLM tool description.

Registration is two marked lines per feature in `features/mod.rs`, inserted by
`cargo xtask new-feature`. The router is built from the registry's manifests at
startup. Features discovered at run time (n8n workflows, hub tools, later MCP)
implement `DynamicSource`.

Features receive everything through `Ctx` (config, secrets, http, clock, store,
scheduler, announcer, log). They own no statics and spawn no threads.

During migration the old `ToolRegistry` and `phrases.rs` stay as a shim consulted
after the new registry.

## Consequences

- "Zero edits to engine or router" holds, with one honest exception: the registry
  file gets two lines.
- The `Intent` enum, `intent_tag`, `EXEMPLARS`, `execute_intent`, and
  `legacy_try_handle` are deleted at the end of the migration.
- Embedding cache keys must derive from the manifest set, since exemplars are data.
- Startup validates manifests; a duplicate id or priority stops the engine with a
  clear message (exit 64).

## Enforced by

Architecture rules 6, 8, 9; the agent-ability test rows "add a tool" and "add a
phrase variant"; the `Intent` variant count ratchet.
