# ADR 0016: Permission tiers and confirmation

- Status: Accepted
- Date: 2026-11-14
- Related: roadmap section 2; findings 6; phases 09, 26

## Context

Every tool runs the moment a phrase matches. `KuroTool.is_local_only` exists but is
never read. Planned features raise the stakes: sending messages, running scripts,
writing calendars, and an LLM that can call tools. The API can also trigger tools
with only the token.

## Options

1. No tiers; trust the phrase match.
2. Confirm everything. Safe and unusable.
3. Four tiers declared in the manifest, with a confirmation policy that depends on
   the tier and on who is asking.

## Decision

Option 3.

| Tier | Meaning | Direct voice command | LLM tool call or API |
|------|---------|----------------------|----------------------|
| `read_only` | no side effect | runs | runs |
| `reversible` | local, undoable | runs | runs, logged |
| `external_send` | leaves the machine to a person or service | runs if the utterance named recipient and content; otherwise asks | always asks |
| `irreversible` | cannot be undone | always asks | always asks |

- The tier is a required manifest field. A feature may raise it per intent.
- Confirmation is a turn-machine state: Kuro states the action in one sentence and
  waits for yes or no; silence or anything else is no.
- Rituals and scheduled jobs run with the tiers of their steps; a ritual that
  contains a step above `reversible` must be confirmed when it is defined, not each
  time it runs.
- Every executed action is appended to an audit log (turn id, source, feature,
  intent, tier, result kind). The log never stores secrets.
- Existing features keep today's behavior: desk power and music are `reversible`;
  WhatsApp and Meet-with-send are `external_send` and already require the recipient
  in the phrase.

Speaker verification is not part of this decision; it was cut from the roadmap.

## Consequences

- LLM tool calling can be enabled without giving the model the ability to send or
  destroy silently.
- A small amount of friction for ambiguous sends.
- Users can lower friction per feature in config, never below the tier floor for LLM
  and API callers.

## Enforced by

Manifest validation (tier required); `agent::tests::tier_matrix`;
`turn::tests::confirmation_timeout_is_no`; the audit log assertions in feature
tests.
