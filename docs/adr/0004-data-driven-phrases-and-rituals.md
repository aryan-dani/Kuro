# ADR 0004: Phrases, replies, and rituals as TOML data

- Status: Accepted
- Date: 2026-11-14
- Related: leaning 3; findings S2, T1; phases 08, 09, 10

## Context

Phrases, replies, the mishearing table, the help text, and the arrival scene are Rust
code, duplicated between `execute_intent` and `legacy_try_handle`, and they carry the
owner's personal wording. The owner leans toward "data over code" in TOML or YAML
validated by schema.

## Options

1. Everything in code (today).
2. Everything in data, including argument extraction and conditional flows.
3. Declarative things in data, logic in code, one data format.

## Decision

Option 3, with TOML only.

In data: exact phrases, semantic exemplars, replies (with named placeholders), help
text, STT bias terms, mishearing corrections, settings declarations, routing stage
order, and rituals.

In code: argument extraction that needs regular expressions or parsing (WhatsApp
recipient, durations, times), and every side effect.

Rituals are deliberately weak: a phrase list, a cooldown, a reply, an optional event
trigger, and a linear list of feature calls with an optional `parallel` flag. No
conditions, loops, or variables. Anything richer is a feature.

YAML is rejected: a second format doubles the validation surface, and its implicit
typing is a known source of silent errors. TOML is already the Rust ecosystem
default and needs one new dependency (`toml`).

Validation happens at load with messages that name file, key, and reason. The
"schema" is the Rust loader plus a generated JSON schema for the shell and docs.

## Consequences

- Phrase and reply edits need no Rust change and no recompilation of logic; they
  still ship in the binary by `include_str!`, with profile files layered on top.
- Personal wording moves to the owner's profile. The example profile is neutral.
- Reply text is no longer greppable in Rust; `cargo xtask check arch` rule 9 verifies
  that every reply key used in code exists in the manifest.
- Thresholds and stage order become visible data, so a change to them is a reviewable
  diff with a corpus diff beside it.

## Enforced by

Manifest validation at startup and in `xtask` (rule 9); the golden corpus; profile
validation in `kuro doctor`.
