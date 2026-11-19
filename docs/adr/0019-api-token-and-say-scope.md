# ADR 0019: API token generation and `/say` scope

- Status: Accepted
- Date: 2026-11-14
- Related: findings 3, 5, 6; constraint "localhost API and token auth"; phase 03

## Context

The localhost API can switch desk power, send WhatsApp messages, and change
settings. Its only protection is `X-Kuro-Token`.

- `kuro-core/src/token.rs` `random_hex` is a linear congruential generator seeded
  from time and pid that emits the low byte of its state. The whole token follows
  from the first byte: 256 possible tokens. When the shell starts first it writes a
  cryptographically random token instead, so real exposure depends on start order.
- `POST /say` runs `kuro_router::try_handle(text)`, which executes real tools, on
  the async runtime thread. Only the gate smoke test and unit tests call it; the
  shell does not.

## Options for the token

1. Operating system generator through the `getrandom` crate.
2. Windows `BCryptGenRandom` through `windows-sys`. No new crate, not portable.

## Options for `/say`

1. Leave it executing.
2. Remove it.
3. Dry run by default; execution is an explicit opt-in with a tier limit.

## Decision

Token: option 1. 32 random bytes, hex encoded. `getrandom` is tiny, ubiquitous, and
keeps `kuro-core` portable. An existing strong token file is reused. A token made by
the old generator is detectable exactly (it equals one of the 256 sequences the old
function can emit), so `ensure_token` replaces a weak token on startup. The shell
client re-reads the token file once when it receives 401, so a rotation does not
strand a running shell. The comparison stays constant-time.

`/say`: option 3.
- The request gains `mode`: `dry_run` (default) or `execute`.
- `dry_run` returns what would happen (`matched`, intent, tool, arguments) and
  executes nothing.
- `execute` works only when `api.say_execute` is true (default false), runs on a
  blocking worker, and is limited to `read_only` and `reversible` tiers. Higher tiers
  are refused with a typed error.
- Until tiers exist (phase 09), phase 03 implements the same rule with a fixed list:
  `send_whatsapp`, `make_meet`, and `run_workflow` are refused through `/say`.
- The gate smoke uses `execute` against an isolated engine with no credentials.

The API continues to bind 127.0.0.1 only and to require the token on every route,
the pipe, and the WebSocket.

## Consequences

- A local process that guesses or reads nothing can no longer brute-force the token.
- A stolen token can read state and flip reversible things, but cannot send messages
  through `/say`.
- One new dependency (`getrandom`).
- Two `kuro-ipc` tests change on purpose and gain regression tests.

## Enforced by

`token::tests::tokens_are_unique_and_long`; `ipc::tests::say_defaults_to_dry_run`;
`ipc::tests::say_refuses_send_tier`; PROTECTED status of `token.rs` and the auth
code.
