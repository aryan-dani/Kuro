# Phase 03: Security fixes

- Size: M (four parts, one PR each)
- Depends on: phases 00, 01
- Behavior change: yes, deliberate (listed below)
- ADRs: 0019
- Protected paths touched: `kuro-rs/crates/kuro-core/src/token.rs`,
  `kuro-rs/crates/kuro-ipc/src/lib.rs`, `scripts/Test-KuroGate.ps1` or the smoke in
  `xtask`. Trailer `ADR: 0019`.

## Goal

Close the security defects found in the audit without changing any voice behavior.

## Deliberate behavior changes and their regression tests

| Fix | Finding | Test |
|-----|---------|------|
| Token from the OS generator; weak tokens rotated | 3 | `token::tests::tokens_are_unique_and_long`, `token::tests::weak_token_is_replaced` |
| Shell re-reads the token once on 401 | 3 | `EngineClientTests.RereadsTokenOnUnauthorized` |
| `/say` defaults to dry run; send-type tools refused | 6 | `ipc::tests::say_defaults_to_dry_run`, `ipc::tests::say_refuses_send_tools` |
| `/say` no longer blocks the async runtime | 6 | `ipc::tests::status_answers_during_slow_say` |
| `speak_sapi` removed | 4 | compile; `rg speak_sapi` empty |
| Meet expiry compares real instants | 7 | `meet::tests::expiry_uses_real_time` |
| Tuya nonces are random, sequence increases | 5 | `tuya::tests::nonce_never_repeats`, `tuya::tests::seq_increases` |

## Context card

Read:
- `docs/adr/0019-api-token-and-say-scope.md`
- `kuro-rs/crates/kuro-core/src/token.rs` (whole file, 54 lines)
- `kuro-rs/crates/kuro-ipc/src/lib.rs`: `authed`, `say`
- `kuro-rs/crates/kuro-ipc/tests/parity.rs` (the `/say` test near line 106)
- `kuro-rs/crates/kuro-tools/src/tuya.rs` (`control_frame`, about line 207),
  `plug.rs` (`send_switch_34`, about lines 87 to 90),
  `kuro-rs/crates/kuro-tools/tests/tuya_framing.rs`
- `kuro-rs/crates/kuro-tools/src/meet.rs` (`token_expired`, about 285 to 298)
- `kuro-rs/crates/kuro-tts/src/lib.rs` (`speak_sapi`, about line 373)
- `KuroShell/KuroShell/EngineClient.cs`

Edit: the files above, `kuro-rs/Cargo.toml` (`getrandom` in workspace
dependencies), the smoke step of the gate.

Do not touch: `kuro-hotkey/**`, routing code, `EngineSupervisor.cs`, the corpus.

Commands: `cargo xtask gate fast`; `cargo xtask gate full` before push.

## Part A: token

Tests first:
- 1000 generated tokens are distinct, 64 characters, lowercase hex.
- `is_weak_token` is true for output of the old algorithm for each of the 256 low
  seed bytes, and false for 1000 new tokens.
- `ensure_token` keeps a strong existing token and replaces a weak one.

Steps:
1. Keep the old algorithm as a private test-only function to generate the 256 weak
   sequences.
2. New `random_hex`: 32 bytes from `getrandom`.
3. `ensure_token`: create if missing; replace if weak; otherwise load.
4. `EngineClient.cs`: on HTTP 401 or pipe `DENY`, re-read the token file once and
   retry. Put the retry decision in a pure helper in `KuroShell.Core` with a test.

## Part B: `/say`

Tests first: a request without `mode` executes nothing (recording executor sees no
call) and returns `dry_run: true` with `matched`; `mode: "execute"` with the setting
off is refused with 403; with the setting on, `pause the song` executes and
`send my clipboard to sam on whatsapp` is refused; `/status` answers while a slow
`/say` is in flight.

Steps:
1. Request body gains optional `mode`. Response keeps `matched` and `reply` and adds
   `dry_run`.
2. Dry run uses `kuro_router::route_semantic(&normalize(text))` for `matched`.
   Pattern-only phrases report `matched: false` until phase 09 provides a pure
   decision; document this in the handler comment.
3. Execution is allowed only when env `KURO_SAY_EXECUTE` is `1` (becomes the setting
   `api.say_execute` in phase 07) and runs in `tokio::task::spawn_blocking`.
4. Refuse `send_whatsapp`, `make_meet`, and `run_workflow` through `/say`: check the
   decided intent before executing.
5. The gate smoke sets `KURO_SAY_EXECUTE=1` for its isolated engine.
6. Update `contract/v1/say.*.json` with `KURO_BLESS=1` and review the diff.

## Part C: dead code and Meet expiry

1. Delete `speak_sapi` after confirming no caller.
2. `token_expired`: parse the stored expiry with `chrono` (RFC 3339, `Z` or numeric
   offset), compare instants with 60 s skew, treat malformed or missing as expired.
   Tests: valid, expired, inside skew, both offset forms, malformed.

## Part D: Tuya nonces

Tests first (`tuya_framing.rs`): two frames built from the same input have
different nonces; decode(encode(x)) still round-trips; sequence numbers increase per
connection; frame layout (header, lengths, tag position) is unchanged.

Steps:
1. GCM nonce: 12 random bytes per frame. Local session nonce in `plug.rs`: 16 random
   bytes per session.
2. Sequence number: a counter owned by the connection, starting at 1.
3. Keep the previous behavior behind env `KURO_TUYA_LEGACY_NONCE=1` for one release,
   in case a device rejects the change.

Hardware confirmation (owner, before merge): desk on, desk off, and the arrival
ritual with the real plug, five times each. The agent cannot verify this part.

## Acceptance criteria

- All tests in the table pass; the corpus is unchanged.
- `/say` contract fixtures changed only in the reviewed, intended way.
- The owner confirms the hardware checklist for part D in the PR.
- An engine started with an old weak token file ends up with a strong token, and a
  running shell reconnects without a restart.

## Rollback

Each part reverts independently. For part D, the legacy env switch is the first
resort before a revert.

## Guardrails

- Never log a token, a Tuya key, or an OAuth token, in tests either.
- Do not change the header name `X-Kuro-Token`, the port, or the bind address.
- No voice-path behavior change: a spoken "send ... on WhatsApp" works as before.
