# 03 Tests and CI

Principle: behavior is frozen by tests before any code moves. Tests are deterministic,
parallel-safe, and need no hardware unless they live in the hardware tier.

## 1. Golden corpus (characterization first, phase 00)

Location: `kuro-rs/fixtures/phrases/*.toml`. The prompt suggested
`tests/fixtures/phrases/`; the corpus sits inside the Rust workspace so tests and
`xtask` resolve it without leaving `kuro-rs/`.

```toml
[[case]]
id = "power-on-plain"
input = "Desk on."
normalized = "desk on"
stage = "exact"                 # exact | semantic | pattern | llm | none
intent = "PowerOn"              # Intent variant today; manifest id after migration
tool = "control_smart_plug"     # omitted when no tool is called
args = { action = "on" }
reply = "Desk on."              # or reply_contains = "..."
needs_model = false             # true: runs only when the embed model is present
```

Files and what they must cover (seeded from the existing tests named in brackets):

| File | Coverage |
|------|----------|
| `normalization.toml` | punctuation, case, contractions, every mishearing correction [`normalization_matrix.rs`] |
| `route_order.toml` | phrases that two stages could claim; cooldown replies [`route_order.rs`] |
| `desk_power.toml` | on, off, polarity words, good night |
| `music.toml` | pause, resume, stop, and near misses |
| `news_inbox.toml` | news and inbox phrasings |
| `meet.toml` | create, send to contact, repeats within cooldown |
| `whatsapp.toml` | the full table in `kuro-tools/tests/parity.rs` 237 to 251 plus failures |
| `workflows.toml` | run by name, alias, unknown name |
| `help_adversarial.toml` | long "can you help me ..." lines must reach the LLM [`semantic_router.rs`] |
| `clipboard_vs_power.toml` | "send my clipboard ... on WhatsApp" is not desk power [`kuro-router/tests/parity.rs`] |
| `smalltalk.toml` | cancel, thanks, identity, time, date, questions |
| `rituals.toml` | the arrival ritual and its cooldown |

Target: at least 250 cases at the end of phase 00. Every later bug fix adds a case.

Harness: `kuro-router/tests/corpus.rs`, one `#[test]` that runs every case in order
in its own test binary. It installs a recording executor, scrubs `TUYA_*`,
`KURO_N8N_*`, `KURO_HUB_*`, `DADDY_HOME_SONG_PATH`, and points `KURO_DATA_DIR` at a
temp directory before the first case, so no real device can be reached (finding N20).
The expected values are recorded from current behavior, reviewed once by the owner,
and then frozen. `KURO_BLESS=1` rewrites expectations; the diff is reviewed in the PR.

After phase 08 the owner-specific cases move to the owner's profile
(`profile/tests/routing.toml`) and run through `kuro doctor --profile-tests`.

## 2. Test pyramid

| Level | What | Where | Tier |
|-------|------|-------|------|
| 1 Unit, pure | normalize, guards, decide, parsers, formatters, `step` | each crate | fast, Linux |
| 2 Feature with fakes | each feature against FakeHttp, FakeClock, FakeStore | `features/<id>/tests.rs` | fast, Linux |
| 3 Turn scenarios | whole turns headless with fakes | `kuro-turn/tests/` | fast, Linux |
| 4 Contract | Rust serializes fixtures, C# reads them | `kuro-protocol/tests/`, `KuroShell.Tests` | full |
| 5 Engine smoke | real binary, isolated port and data dir | `xtask smoke` | full |
| 6 Hardware | real STT on fixture audio, TTS synth, device loopback, hotkey injection | `xtask gate hardware` | opt-in |

Fakes live in `kuro-core/src/testkit/` behind the `testkit` cargo feature: `FakeMic`,
`WavMic`, `FakeStt`, `FakeTts`, `FakeLlm`, `FakeClock`, `FakeHttp`, `FakeStore`,
`RecordingHud`, and a `Scenario` builder.

Golden helper: `kuro_core::testkit::assert_golden(path, actual)` with `KURO_BLESS=1`.
No snapshot library is added.

## 3. WAV replay harness

- `kuro-rs/fixtures/audio/*.wav`: about 40 clips of corpus commands, 16 kHz mono,
  under 3 MB total, synthesized by `cargo xtask gen audio-fixtures` with Kuro's own
  TTS. `fixtures/audio/LICENSE.md` records the voice model and its license (verify
  the voice license before committing clips).
- `fixtures/audio/expected.toml`: clip, expected text, expected intent.
- Fast and full tiers: `WavMic` feeds the clip, `FakeStt` returns the expected text,
  so the turn scenario runs without a model.
- Hardware tier: the same scenarios with the real STT backend.
- Required housekeeping: `.gitignore` and the invariant that fails on tracked `.wav`
  files both need an exception for `kuro-rs/fixtures/audio/`.
- Synthetic speech does not represent a real voice or microphone. Owners can add
  private recordings under `profile/tests/audio/` (never tracked); the hardware tier
  runs those too.

## 4. STT quality regression (hardware tier)

Metrics on the fixture set: word error rate after normalization, and command accuracy
(the transcript routes to the expected intent). Thresholds are ratchets: phase 20
measures, records the numbers in `kuro-rs/xtask/data/ratchets.toml`, and the job fails
when WER rises by more than 2 points or command accuracy falls by more than 2 points.
Starting expectation, unverified: command accuracy at least 95 percent on synthetic
clips.

## 5. Determinism rules

- No global state (architecture rule 3) and no env mutation: config is injected.
- No wall clock and no sleep: `Clock` port, `FakeClock::advance`.
- No network: `FakeHttp`, or a loopback listener on port 0 for adapter tests.
- Unique temp directory per test; nothing under the real `%LOCALAPPDATA%\Kuro`.
- The suite must pass with default parallelism and with `--test-threads=1`.
- A test that needs a model or tool calls `require_model!("kokoro")` or
  `require_tool!("espeak-ng")`: skipped and counted in fast and full, failing in
  hardware. Silent early returns (finding N24) are removed.

## 6. Property and fuzz tests

`proptest` as a dev-dependency (widely used, no runtime cost). `cargo-fuzz` is not
adopted: it needs nightly and is awkward on Windows.

| Target | Properties |
|--------|------------|
| `normalize` | idempotent; never panics on arbitrary Unicode; output is lowercase words and spaces |
| matchers and feature `parse` | never panic; a match implies the intent's required words are present |
| duration and time parsers | parse(format(x)) equals x |
| Tuya framing | decode(encode(x)) equals x; decode never panics on arbitrary bytes; nonces differ |
| config loader | arbitrary TOML never panics; unknown keys are rejected with the key name |
| protocol types | deserialize(serialize(x)) equals x; unknown `type` is ignored, not fatal |
| turn `step` | from any state, Cancel reaches Idle; the epoch never decreases |

The existing `kuro-tools/tests/fuzz_tools.rs` stays and moves with its features.

## 7. Failure-injection suite

`FakeHttp` behaviors: `Refuse`, `Hang`, `Status(401)`, `Status(500)`, `Garbage`,
`SlowBody`. Device fakes: `Busy`, `PermissionDenied`, `Gone`. Each feature must cover
every row of its table in [00-failure-modes.md](00-failure-modes.md). A check enforces
names: a feature whose manifest requires `http` must have tests ending in
`_times_out`, `_unreachable`, and `_malformed`. One engine-level test injects a panic
inside a fake feature and asserts that the next turn still works.

## 8. Coverage bars

`cargo-llvm-cov` in the Linux job; `coverlet.collector` for `KuroShell.Tests`. Bars
are ratchets that start at the measured value when each crate is created or first
measured, with these targets:

| Crate | Target line coverage |
|-------|----------------------|
| kuro-turn, kuro-protocol | 90 percent |
| kuro-router, kuro-config, kuro-core | 85 percent |
| kuro-features | 80 percent |
| KuroShell.Core | 80 percent |
| adapters, engine, WinUI project | no bar; covered by smoke and hardware tiers |

## 9. GitHub Actions

`ci.yml` (push and pull request), `concurrency` cancels superseded runs:

| Job | Runner | Steps |
|-----|--------|-------|
| `portable` | ubuntu-latest | `xtask check` (text, size, arch, `gen --check`), `cargo test --locked` for the portable crate list, coverage |
| `windows` | windows-latest | `cargo xtask gate full` (clippy, workspace tests, shell build and tests, contract, smoke) |
| `protected` | ubuntu-latest, PR only | `cargo xtask check protected --base origin/main` |
| `secrets` | ubuntu-latest, PR only | gitleaks on the PR range |

The Linux job starts with the crates that build there today and the list only grows
(a ratchet in `kuro-rs/xtask/data/portable_crates.txt`). It sets `CFLAGS` and
`CXXFLAGS` explicitly because `kuro-rs/.cargo/config.toml` injects MSVC flags
(finding N17).

`scheduled.yml` (weekly and manual): `cargo audit`, `cargo deny check`, link check,
gitleaks on full history, `models.lock.toml` URL liveness.

`hardware.yml` (manual; optional self-hosted runner): `cargo xtask gate hardware`. It
refuses to run unless `KURO_DATA_DIR` and `KURO_API_PORT` point away from the
defaults.

Caching: `Swatinem/rust-cache` (already used), NuGet cache, the LLVM download.
No job and no script may kill or restart a developer's Kuro: `Stop-LockingKuro` and
the restart block in `Test-KuroGate.ps1` are deleted in phase 01.

## 10. Supply chain

| Item | Choice | Why |
|------|--------|-----|
| Toolchain | `kuro-rs/rust-toolchain.toml` pinned to the stable release CI uses, with rustfmt and clippy | reproducible lints |
| Licenses and bans | `cargo-deny` with `kuro-rs/deny.toml` | standard tool; allow-list in file 04 |
| Advisories | `cargo audit`, weekly and on lockfile change | standard tool |
| Updates | Dependabot for cargo, nuget, github-actions, weekly, grouped | built into GitHub; Renovate needs an app install |
| Lockfiles | `Cargo.lock` (tracked) with `--locked`; NuGet lock file for the shell | reproducible builds |
| Actions | pinned by commit SHA | supply chain hygiene |
| Models | `models.lock.toml` with sha256; `kuro doctor --fetch-models` verifies | downloads are unchecked today |
| Secrets | gitleaks in CI plus the existing regex check in the fast gate | defense in depth |

## 11. `cargo xtask` and wrappers

One Rust binary, no product-crate dependencies, argument parsing by hand (no new
dependency). A root `.cargo/config.toml` alias makes `cargo xtask` work from the repo
root.

| Command | Purpose |
|---------|---------|
| `gate fast`, `gate full`, `gate hardware` | the three tiers |
| `check text`, `size`, `arch`, `protected`, `links` | individual checks |
| `gen [--check]` | repo map, config docs and code, feature docs, CODEOWNERS |
| `new-*` | scaffolders (file 02) |
| `bench router`, `stt`, `tts`, `turn` | repeatable benchmarks, JSON output |
| `smoke` | start the engine on an isolated port, probe, shut down |
| `build`, `install` | release build; copy binaries to `%LOCALAPPDATA%\Kuro\bin` |

Thin wrappers kept for muscle memory: `scripts/Test-KuroGate.ps1` (calls
`gate full`), `scripts/Test-KuroInvariants.ps1` (calls the checks),
`scripts/Build-KuroNative.ps1` (calls `build`). `.githooks/pre-commit` runs
`gate fast`; a new `.githooks/pre-push` runs `gate full`.
