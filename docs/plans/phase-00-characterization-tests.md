# Phase 00: Characterization tests

- Size: M (three parts, one PR each)
- Depends on: nothing
- Behavior change: none. Tests and fixtures only.
- ADRs: 0009 (contract fixtures), 0015
- Protected paths touched: none

## Goal

Freeze today's observable behavior so that every later phase can prove it changed
nothing by accident: what each phrase routes to and replies, what the API returns,
and what a voice turn does in order.

## Scope

In:
- Part A: golden routing corpus (at least 250 cases) and its harness.
- Part B: protocol snapshot fixtures under `contract/v1/` and a Rust test.
- Part C: voice-turn snapshot test and a baseline measurements file.

Out: any change under a `src/` directory, any script change, the Linux CI job
(phase 01), fixing defects the corpus reveals.

## Context card

Verify these anchors exist before starting; if one is missing, stop and report.

Read:
- `kuro-rs/crates/kuro-router/tests/parity.rs`, `route_order.rs`,
  `normalization_matrix.rs`, `semantic_router.rs` (existing assertions to copy)
- `kuro-rs/crates/kuro-tools/tests/parity.rs` (WhatsApp table, about lines 237 to 251)
- `kuro-rs/crates/kuro-tools/src/lib.rs`: `set_executor`, `execute_tool`
- `kuro-rs/crates/kuro-router/src/lib.rs`: exported names (`normalize`, `try_handle`,
  `route_semantic`)
- `kuro-rs/crates/kuro-ipc/tests/parity.rs` (how tests call the router with `call`)
- `kuro-rs/crates/kuro-engine/tests/loop_stages.rs`
- `docs/plans/03-tests-and-ci.md` sections 1 and 5

Create:
- `kuro-rs/fixtures/phrases/*.toml` (12 files listed in file 03 section 1)
- `kuro-rs/crates/kuro-router/tests/corpus.rs`
- `contract/v1/*.json`, `contract/README.md`
- `kuro-rs/crates/kuro-ipc/tests/contract_snapshot.rs`
- `kuro-rs/crates/kuro-engine/tests/voice_turn_snapshot.rs`
- `docs/plans/baseline-measurements.md`

Edit: `Cargo.toml` of `kuro-router`, `kuro-ipc`, `kuro-engine` (dev-dependencies
only; add `toml` to `[workspace.dependencies]`).

Do not touch: any file under `src/`, `scripts/`, `KuroShell/`, `.github/`.

Commands:
```powershell
cd kuro-rs
cargo test -p kuro-router --test corpus
cargo test -p kuro-ipc --test contract_snapshot
cargo test -p kuro-engine --test voice_turn_snapshot
cargo clippy --workspace --all-targets -- -D warnings
```
The pre-commit hook still runs the old gate, which stops a running Kuro. Tell the
owner before committing.

## Part A: golden corpus

1. Write the harness first with three hand-written cases and see it pass.
2. Harness rules: one `#[test]`; before the first case remove every env var that
   starts with `TUYA_`, `KURO_N8N_`, `KURO_HUB_`, `N8N_`; set `DADDY_HOME_SONG_PATH`
   to a path that does not exist; set `KURO_DATA_DIR` to a fresh temp directory;
   install a recording executor with `set_executor` that returns a fixed string per
   tool and records `(name, args)`.
3. Per case assert, when the field is present: `normalized`, `intent`
   (`route_semantic` result, `Debug` name), `tool`, `args`, `reply` or
   `reply_contains`, and `handled` (true or false).
4. Cases with `needs_model = true` run only when
   `kuro_router::semantic::find_onnx()` is `Some`; print the number skipped.
5. `KURO_BLESS=1` rewrites `reply` and `normalized` from actual output.
6. Add `known_bug = "<finding id>"` on any case that freezes a known defect, so the
   fix phase can flip it on purpose.
7. Fill the files to at least 250 cases. Sources: every assertion in the test files
   listed above, every exemplar category in `semantic.rs` `EXEMPLARS` (one case per
   intent at least), every entry of the mishearing table in `phrases.rs` `normalize`.
8. Prove the harness can fail: change one expected reply, see the case id in the
   failure, revert.

Cooldown-sensitive cases (the arrival ritual, Meet) must run in a fixed order inside
their file; document the order in a comment at the top of the file.

## Part B: protocol snapshot

1. For each route in `kuro_ipc::router` (`/status`, `/config` GET and PUT,
   `/workflows`, `/contacts`, `/timing`, `/say`, `/thread/clear`, `/dismiss`,
   `/shutdown`) capture the response for a default `AppState` built the way
   `kuro-ipc/tests/parity.rs` builds it.
2. For `event_frame`, capture one frame per `HudState` value, with and without
   transcript text.
3. Store canonical JSON (sorted keys, volatile values such as uptime and pid replaced
   by `"<volatile>"`) in `contract/v1/<name>.json`.
4. The test rebuilds each value and compares with the file. `KURO_BLESS=1` rewrites.
5. `contract/README.md`: what the files are, that they are PROTECTED from phase 01
   on, and that the C# side joins in phase 15.

## Part C: voice-turn snapshot and baseline

1. `voice_turn_snapshot.rs`: call `VoiceTurn::run` (the public entry in
   `kuro-engine/src/voice.rs`) for 10 inputs with playback off, the recording
   executor installed, and `ollama_up = false`. Record reply text and the ordered
   list of timing stage names. Compare with an inline expected table.
2. `docs/plans/baseline-measurements.md`: record, with the command used and the
   machine class (no personal identifiers): old gate duration locally and in CI, test
   count, number of tests that return early because a model or espeak is missing,
   release build time cold and warm. Do not record numbers you did not measure.

## Acceptance criteria

- At least 250 corpus cases; every file in the list exists and has at least 8 cases.
- All three test binaries pass locally and in CI; CI prints the skipped model cases.
- `git diff --stat` shows no change under any `src/` directory.
- The owner has read the frozen replies once and ticked the PR checklist item
  "expected values reviewed".
- No test opened a socket to a non-loopback address (the env scrub assertion inside
  the harness passes).

## Rollback

Revert the PRs. Nothing else depends on these files until phase 01.

## Guardrails

- Do not "fix" odd behavior you find. Freeze it and tag it `known_bug`.
- No sleeps, no network, no real devices, no writes outside temp directories.
- No emoji, no em dashes. Contact names in new fixtures are neutral (`alex`, `sam`).
