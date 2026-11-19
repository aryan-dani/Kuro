# 07 First 12 agent tasks

Small, safe, independent tasks to hand to a Cursor agent right after approval. Each
is one PR. They are slices of phases 00 to 05, so finishing them shortens those
phases. Run them in this order; tasks 5 to 12 can run in any order after task 4.

Rules for every task: read `docs/plans/README.md` section "Rules for executing
agents" first; write the test before the change; no emoji, no em dashes; run
`.\scripts\Test-KuroGate.ps1` until task 2 lands, then `cargo xtask gate fast`; if
the code does not match the task text, stop and report.

Paths are relative to the repo root. Line numbers are hints from commit `f661122`.

## 1. Golden corpus harness (slice of phase 00)

Create `kuro-rs/fixtures/phrases/` with `normalization.toml`, `route_order.toml`,
`clipboard_vs_power.toml`, and `help_adversarial.toml`, at least 40 cases in total,
copied from the assertions in `kuro-router/tests/normalization_matrix.rs`,
`route_order.rs`, `parity.rs`, and `semantic_router.rs`. Add
`kuro-router/tests/corpus.rs`: one `#[test]` that removes `TUYA_*`, `KURO_N8N_*`,
`KURO_HUB_*`, sets `DADDY_HOME_SONG_PATH` to a missing path and `KURO_DATA_DIR` to a
temp directory, installs a recording executor with `kuro_tools::set_executor`, then
checks `normalized`, `intent`, `tool`, `args`, and `reply` for each case. Support
`KURO_BLESS=1`. Do not change any product code. Add `toml` as a dev-dependency.
Acceptance: `cargo test -p kuro-router --test corpus` passes; changing one expected
reply in a fixture makes it fail and names the case id.

## 2. `xtask` skeleton with the size ratchet (slice of phase 01)

Add `kuro-rs/xtask` to the workspace with `check size` and the data file
`kuro-rs/xtask/data/size_allowlist.toml` holding the baseline in
`docs/plans/02-guardrails.md` section 6. Add the alias in a root
`.cargo/config.toml` and in `kuro-rs/.cargo/config.toml`. Call it from
`scripts/Test-KuroInvariants.ps1`. No other checks yet.
Acceptance: the tree passes; a new 401-line `.rs` file fails; adding one line to
`kuro-router/src/phrases.rs` fails; unit tests in `xtask` cover the three rules.

## 3. Em dash cleanup and check (slice of phase 01)

Replace every em dash (U+2014) and en dash (U+2013) in tracked text files with a
comma, colon, or hyphen. In `kuro-tts/src/kokoro_vocab.rs` the character is data:
write it as the escape `\u{2014}` so the vocabulary is unchanged. Add
`cargo xtask check text` (emoji and dash scan over the same extensions the invariant
script uses, plus `.txt`) and call it from `Test-KuroInvariants.ps1`.
Acceptance: `rg` finds neither character in tracked files; the Kokoro tokenizer
tests pass unchanged; a file containing an em dash fails the check with path and
line.

## 4. `AGENTS.md` v0 (slice of phase 01)

Write `AGENTS.md` at the root from the template in `docs/plans/02-guardrails.md`
section 1, limited to commands that exist today (`Test-KuroGate.ps1`,
`cargo xtask check size`, `cargo xtask check text`) and a "where to work" table for
the current layout. Link `docs/plans/README.md`.
Acceptance: under 80 lines; passes `check text`; every path named in it exists.

## 5. Pin the toolchain (slice of phase 01)

Add `kuro-rs/rust-toolchain.toml` pinned to the stable version CI currently
resolves, with `rustfmt` and `clippy`.
Acceptance: `rustup show` inside `kuro-rs` reports the pinned version; CI is green.

## 6. `.gitignore` repair (slice of phase 05)

Apply section 10 of `docs/plans/04-repo-hygiene.md`: remove the blanket `*.png`
rule, ignore the known scratch files and folders, allow
`kuro-rs/fixtures/audio/*.wav`.
Acceptance: `git ls-files` is unchanged; `git check-ignore -q
KuroShell/KuroShell/Assets/Any.png` exits 1; `git status` shows no scratch files.

## 7. `LICENSE` (slice of phase 05)

Add the MIT license text at the root with the copyright holder the owner confirms
(ask if unknown). Rename `KuroShell/KuroShell/Assets/Fonts/OFL.txt` to
`Poppins-OFL.txt` and update any project reference.
Acceptance: the shell builds; the font invariant still passes; GitHub shows MIT.

## 8. Delete dead Python and stale docs (slice of phase 05)

Delete `scripts/bench/*.py` except `gen_kokoro_vocab.py`, `bench/`,
`voices/README.md`, `legacy/README.md`. Remove the bench note from
`CONTRIBUTING.md`. Narrow the Python allow-list in `Test-KuroInvariants.ps1` to
`scripts/build_icons.py` and `scripts/bench/gen_kokoro_vocab.py`. Remove the
`run_stt.py` entry from the size allowlist.
Acceptance: the gate passes; `git ls-files bench voices legacy` prints nothing.

## 9. Delete `speak_sapi` (slice of phase 03)

Remove the unused `pub fn speak_sapi` from `kuro-tts/src/lib.rs` (about line 373).
Confirm first that no caller exists.
Acceptance: `rg speak_sapi` finds nothing; clippy and tests pass.

## 10. Cryptographic API token (slice of phase 03; PROTECTED, trailer `ADR: 0019`)

In `kuro-core/src/token.rs`, replace `random_hex` with 32 bytes from the operating
system generator (`getrandom`, added to the workspace dependencies), hex encoded to
64 characters. Keep `ensure_token` semantics: an existing token file is reused.
Tests first: 1000 generated tokens are distinct, 64 characters, lowercase hex; an
existing file is returned unchanged.
Acceptance: the new tests pass; `kuro-ipc` auth tests pass unchanged.

## 11. Timeouts on unbounded network calls (slice of phase 02)

Give `inbox_run` (8 s) and `screen_run` (30 s) in `kuro-tools/src/feeds.rs`, and
the OAuth refresh and create-space calls in `kuro-tools/src/meet.rs` (8 s each),
explicit connect and read timeouts. Add a seam so tests can point each call at a
loopback listener that accepts and never answers.
Tests first: each call returns an error within its timeout plus one second.
Acceptance: three new tests pass; success-path tests are unchanged; the corpus is
unchanged.

## 12. Meet token expiry with real time (slice of phase 03)

Rewrite `token_expired` in `kuro-tools/src/meet.rs` (about lines 285 to 298) to
parse the stored expiry with `chrono` and compare instants with a 60 second skew. A
malformed or missing expiry counts as expired.
Tests first: valid, expired, inside the skew, `Z` and numeric offset forms,
malformed.
Acceptance: the new tests pass; no other behavior changes.
