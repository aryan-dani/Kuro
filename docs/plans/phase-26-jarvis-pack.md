# Phase 26: Jarvis pack (confirm, system, brief, discoverability)

- Size: L (four parts, one PR each preferred)
- Depends on: phase 25
- Behavior change: yes. Ships the remaining v1 Jarvis shortlist on top of the
  extension model. Defaults stay safe (agent off, confirm on for send/irreversible).
- ADRs: 0011, 0016, 0008 (Confirming state), 0019
- Protected paths: turn machine for Confirming. Trailer `ADR: 0016`, `ADR: 0008`.

## Goal

Make Kuro feel like a desk Jarvis: confirm dangerous plans, let the LLM call
tools within policy, control the PC with voice, deliver a morning brief, and
answer "what can I say" with a health page. Every part must fit one feature folder
or one documented core extension; no drive-by engine edits.

## Scope

| Part | Delivers | Core touch? |
|------|----------|-------------|
| A | Confirmation policy + LLM tool calling | yes: `Confirming` state, agent loop |
| B | Computer control basics (volume, media, lock, open app) | no: feature only |
| C | Morning brief ritual + weather feature | no: feature + ritual TOML |
| D | "What can I say", feature toggles, health page | shell page + feature |

Out: full MCP runtime, cloud wake word, email write, calendar write, training UI.

## Context card

Read:
- `docs/plans/05-jarvis-roadmap.md` items 4, 6, 7, 8 and section 2 tiers
- `docs/adr/0011-llm-provider-port.md`, `0016-permission-tiers-and-confirmation.md`
- Turn states from phase 12/20; Announcer/Scheduler from 22; calendar/timers from
  23/24; protocol `card` from 15; Pages registry from 17
- Existing LLM call sites in engine/router

Create:
- Turn `Confirming` + agent step cap
- `features/system/` (part B)
- `features/weather/` + ritual `morning_brief` (part C)
- `features/help_discover/` or extend help; shell `Pages/Health/`, `Pages/Phrases/`
  (part D)
- Audit log rows for every agent/tool action
- Corpus cases for new phrases; scenario tests for confirm yes/no/timeout

Edit: turn machine (A only); registry lines; example profile flags default safe;
AGENTS.md where-to-work if new folders appear.

Do not touch: hotkey LL-hook; music player internals; weakening `/say` (0019).

Commands:
```powershell
cargo test -p kuro-turn
cargo test -p kuro-features
cargo test -p kuro-llm
dotnet test KuroShell/KuroShell.Tests
cargo test -p kuro-router --test corpus
cargo xtask gate full
```

## Part A: Confirmation and LLM tool calling

Tests first:
- FakeLlm plans `desk on` then `send whatsapp`: first runs, second enters Confirming;
  "yes" executes, "no" cancels, timeout treats as no.
- Unknown tool name or bad args: reject, speak reason, no panic.
- `agent.max_steps` (default 3) stops loops; audit log has one row per attempt.
- Tier matrix from ADR 0016: read_only and reversible run; external_send and
  irreversible always confirm when source is LLM.

Steps:
1. Add `Confirming { plan_id, tier, summary }` to the turn machine; cancel and
   epoch still win.
2. Agent loop: LLM returns tool calls from feature `llm_description` + slots;
   execute through the same FeatureRegistry as voice.
3. Config: `agent.enabled` (false), `agent.max_steps`, `agent.confirm` by tier,
   `agent.allow_tiers`.
4. Wire audit log (Store): turn id, source, tier, tool, result.
5. Spoken confirm prompt from TOML, not stringly Rust.

Acceptance A:
- Agent off by default; voice path unchanged when agent disabled.
- Confirm tests green; corpus for plain desk phrases unchanged.

## Part B: Computer control basics

Tests first with fake `SystemControl`: volume set/step/mute, media key, lock,
open_app unknown/missing path; routing cases in corpus.

Steps:
1. Scaffold `features/system/` with `win.rs` behind `cfg(windows)`.
2. Config: `system.enabled`, `system.apps` map in profile, `system.volume_step`.
3. Tier reversible; irreversible actions (if any) must confirm.
4. No engine edits beyond registry.

Acceptance B:
- Fake tests only in CI; hardware checklist in feature README for owner.

## Part C: Morning brief and weather

Tests first:
- Weather fixture JSON through FakeHttp; golden speech.
- Ritual `morning_brief`: calendar + weather + inbox + news; one step failure does
  not stop the others; Announcer quiet hours respected.

Steps:
1. `features/weather/` read_only; lat/lon from profile; no API key in repo.
2. Ritual TOML composing existing features; schedule optional via Scheduler.
3. Example profile: weather off until location set.

Acceptance C:
- Brief works with fakes; location unset speaks a clear setup hint.

## Part D: Discoverability and health

Tests first:
- `what can I say` returns a card/list from registered manifests (enabled only).
- Health page reads doctor JSON shape; toggles write profile flags without secrets.
- Shell PageRegistry hosts Health and Phrases without MainWindow bloat.

Steps:
1. Help/discover feature enumerates intents from registry.
2. Pages for Health (doctor summary) and Phrases (searchable list).
3. Toggles for optional adapters (Meet, WhatsApp, hub, wake word, agent) with
   restart-needed hints where required.

Acceptance D:
- New user can discover commands without reading source.
- No personal data on health page; token never displayed in full.

## Cross-part acceptance

- v1 shortlist from docs/plans/README.md items 4, 6, 7, 8 done (1 to 3 and 5 were
  phases 23 to 25).
- Agent-ability: each new capability is a feature or page folder; confirm/agent is
  the only core exception and is documented in ADR 0016.
- Gate full green; corpus includes new cases; no emoji, no em dashes, no secrets in
  tracked profiles.
- Manual owner checklist: confirm a send, volume command, morning brief dry run,
  open Health page, toggle wake word off.

## Rollback

Revert per part. Part A revert must leave voice PTT path identical to phase 25.

## Guardrails

- Never auto-enable agent or wake word in the example profile.
- LLM must not bypass permission tiers.
- Do not expand `/say` beyond ADR 0019.
- Prefer spoken failures over silent no-ops.
- If a part needs a new core port, stop and write an ADR first.
