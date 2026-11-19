# 06 Risks

Ranked by expected damage. "Caught by" names the test or check that turns the risk
into a red gate.

| # | Risk | Why it is likely here | Mitigation | Caught by |
|---|------|-----------------------|------------|-----------|
| 1 | Silent routing behavior change during the restructure | routing order is implicit in `if` chains; replies are duplicated in two functions; thresholds interact | freeze behavior first (phase 00); move one feature per PR; keep the legacy path as a shim until the last feature moves; move-only PRs may not touch the corpus | golden corpus (`kuro-router/tests/corpus.rs`); corpus-unchanged check in refactor PRs |
| 2 | Owner behavior lost when personal data leaves the code | "sir", the arrival ritual, the mishearing table, mic tokens, and default URLs are code today | `kuro init --from-legacy` writes the owner's profile; owner cases move to `profile/tests/routing.toml`; the phase is not done until the owner corpus passes against the owner profile | `kuro doctor --profile-tests`; phase 08 acceptance |
| 3 | Hotkey regression | low-level hook with global state; hard to test; one bad edit kills the product | PROTECTED path; no split of `kuro-hotkey`; the `Trigger` wrapper lives outside it; hardware-tier key injection test; manual checklist in every phase that touches triggers | `kuro-hotkey/tests/parity.rs`; hardware tier `hotkey_hold_release`; protected-path check |
| 4 | Engine and shell version skew | two build systems, hand-copied DTOs, no handshake | fixtures are the contract; handshake refuses a different major version; one version stamp for both | `kuro-protocol/tests/fixtures.rs`; `ContractTests` in `KuroShell.Tests` |
| 5 | Windows APIs and MSVC flags block the Linux job | `kuro-core` depends on `windows-sys` and `ort`; `.cargo/config.toml` injects MSVC `CFLAGS` | Linux crate list is a ratchet that starts with what builds today; explicit `CFLAGS` in the job; `cfg(windows)` code only in `win.rs` files | `portable` CI job; architecture rule 2 |
| 6 | Panic strategy change exposes stuck flags and half-finished state | today a panic kills the process, which hides every cleanup bug | RAII guards before the switch; `catch_unwind` at two boundaries only; panic hook logs; watchdog exits when the turn thread stalls | `engine::tests::panic_in_feature_keeps_ptt`; `meet::tests::busy_released_on_panic` |
| 7 | Model and tool licensing in a public template | Piper voices, espeak-ng, neonize, wake-word models have terms that differ from MIT | repo ships no models or third-party binaries; `models.lock.toml` records license; WhatsApp and Piper are opt-in; license check is a task in phases 05 and 25 | `cargo deny check licenses`; manual checklist in phase 05 |
| 8 | Migration fatigue: the shim and both paths live forever | 9 features to migrate, each with a failure table | one repeatable card (phase 10); a visible counter in `docs/plans/README.md`; ratchets make the old files unable to grow; the final run deletes `kuro-tools` and `phrases.rs` | size ratchet; architecture rule 8; `Intent` enum variant count ratchet |
| 9 | A faster agent executes a phase against code that has moved | line numbers in this plan are from `f661122` | every context card starts with "verify these anchors"; anchors are symbol names, with line numbers only as hints; stop-and-report rule in `AGENTS.md` | review; the phase's "tests written first" step fails early when anchors are wrong |
| 10 | Tests reach real devices on the owner's machine | spawned threads bypass the thread-local test hook | corpus harness scrubs env and data dir; architecture rule against `thread::spawn` in features; fakes through `Ctx` | `corpus.rs` env scrub assertion; architecture rule 6 |
| 11 | Config migration loses local state or secrets | new layout under `%LOCALAPPDATA%\Kuro`, new file names | read old locations for one release; `kuro doctor --migrate` copies, never moves, and prints a report; backup folder with a timestamp | `config::tests::legacy_layout_is_read`; `doctor::tests::migrate_is_copy_only` |
| 12 | Supervisor consolidation leaves Kuro not restarting | three restart paths exist because each covered a gap | write the gap list first (hung engine, shell crash, logon); cover each in the single supervisor or the engine watchdog before deleting a path; Task Scheduler behavior is unverified and is tested first | `SupervisorTests`; manual kill tests in phase 18 |
| 13 | Gate becomes slow again | more checks, more crates | budgets are printed by the gate; fast tier measured in CI; changed-crate selection; model tests only in the hardware tier | gate self-timing; fails above 90 s warm in CI for the fast tier |
| 14 | Generated files drift or are hand-edited | new `gen` step is easy to forget | `gen --check` in fast and full tiers; `// @generated` header; rule file | architecture rule 14 |
| 15 | Over-engineering: ports and manifests nobody needs | 19 ports are listed | a port is added only in the phase that has two implementations (real and fake) and a consumer; phase 22 capabilities land with timers as the first consumer in the next phase | review rule in `AGENTS.md`; unused-port check (a port with no non-test implementation fails) |
| 16 | Always-on microphone (wake word) erodes trust | privacy-sensitive | off by default; visible indicator; audio never stored or sent; one ADR-reviewed code path | `wake_word::tests::disabled_opens_no_stream` |
| 17 | LLM tool calling performs the wrong action | models hallucinate arguments | tier table; confirmation for sends and irreversible actions; step cap; audit log; schema validation of arguments | `agent::tests::tier_matrix`; `agent::tests::bad_arguments_rejected` |
| 18 | Personal data remains in git history of a public repo | old commits contain IPs and paths | publish the template as a fresh repository (open question 2); gitleaks and the personal-pattern check on the new history | scheduled gitleaks; architecture rule 10 |
| 19 | Unverified hardware assumptions | AVX2 flags, Legion gating, NPU and CUDA paths, mic names | document AVX2; `kuro doctor` checks the CPU; delete dead acceleration paths; device names come from config | `doctor::tests::cpu_check_reports_missing_avx2` |
| 20 | Tuya nonce and ack changes break a working plug | cannot be verified without the device | keep the frame layout; property tests on framing; owner runs the hardware checklist before merge; kill-switch to the old behavior for one release | `tuya_framing.rs`; manual checklist in phase 03 |

## Standing mitigations

- Behavior change is allowed only in phases that say so, and each change names its
  regression test.
- Every phase has a rollback: revert the PR. No phase requires a data migration that
  cannot be undone, because migrations copy and keep the original.
- Anything marked "unverified" in this plan is confirmed by the first step of the
  phase that depends on it, or the phase stops and reports.
