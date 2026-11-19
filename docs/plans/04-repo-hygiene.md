# 04 Repo hygiene and template readiness

## 1. License (ADR 0013)

Recommendation: MIT, one `LICENSE` file at the repo root.
- It matches what the code already declares (`license = "MIT"` in
  `kuro-rs/Cargo.toml`).
- It is the least friction for people who click "Use this template".
- It covers Rust, C#, scripts, and docs with one text.

Alternative considered: Apache-2.0 (explicit patent grant) or dual MIT/Apache-2.0
(Rust convention). Rejected for a template because two licenses double the notices
every fork must carry. Open question 1 lets the owner overrule.

## 2. Third-party notices

`THIRD_PARTY_NOTICES.md` at the root, plus `models.lock.toml` carrying a `license`
field per model. The repo distributes source and fonts only. Models, the Piper
executable, espeak-ng, and the neonize DLL are downloaded or installed by the user.

All license identifiers below are from memory and must be verified against the
upstream repositories in phase 05 before the notices file is committed.

| Component | Used as | License (to verify) | Template action |
|-----------|---------|---------------------|-----------------|
| Whisper GGML models | downloaded | MIT | list in `models.lock.toml` |
| Kokoro-82M ONNX | downloaded | Apache-2.0 | list; note voice files |
| bge-small-en-v1.5 | downloaded | MIT | list |
| Silero VAD | downloaded | MIT | list |
| Piper executable and voices | external process | code and each voice differ; newer Piper builds are GPL | optional engine; user installs; voices carry their own terms |
| espeak-ng | external process | GPL-3.0-or-later | not distributed, not linked; optional; document |
| neonize DLL (embeds whatsmeow) | loaded at run time | Apache-2.0 and MPL-2.0 | not distributed; feature off by default; state that it is an unofficial WhatsApp client and may breach WhatsApp terms |
| Space Grotesk, JetBrains Mono, Poppins | bundled fonts | OFL-1.1, texts present | rename `OFL.txt` to `Poppins-OFL.txt`; list in notices |
| Ollama models (for example qwen2.5) | user pulls | varies by model | name no default that the user has not chosen in `kuro init` |
| The arrival song | referenced by filename in `music.rs` | copyrighted recording | remove the filename; the track is a path in the user profile |
| Rust crates | linked | mostly MIT or Apache-2.0 | `cargo-deny` allow-list: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0 |
| Windows App SDK, .NET | linked | MIT | list |
| Wake-word models (phase 25) | downloaded | some popular pretrained sets are non-commercial | choose the model only after a license check |

## 3. Community and governance files

| File | Content | Phase |
|------|---------|-------|
| `LICENSE` | MIT | 05 |
| `THIRD_PARTY_NOTICES.md` | table above, verified | 05 |
| `CONTRIBUTING.md` | rewrite: setup, gates, the tutorial in section 5, PR rules | 05, tutorial in 11 |
| `CODE_OF_CONDUCT.md` | Contributor Covenant 2.1 | 05 |
| `SECURITY.md` | private reporting through GitHub advisories; scope: the localhost API, token, secrets handling; no bounty | 05 |
| `SUPPORT.md` | where to ask, what to include (`kuro doctor --json` output) | 05 |
| `CHANGELOG.md` | Keep a Changelog format, starts at the first template tag | 05 |
| `.github/pull_request_template.md` | checklist: phase or task id, tests first, gate output, behavior change yes or no, ADR trailer if protected | 05 |
| `.github/ISSUE_TEMPLATE/bug.yml`, `feature.yml` | bug asks for doctor output; feature asks for intents and tier | 05 |
| `.github/CODEOWNERS` | generated (file 02) | 05 |
| `.github/dependabot.yml` | cargo, nuget, actions | 05 |
| `ARCHITECTURE.md` | one page: layer diagram, turn data flow, where things live, links to ADRs | 05 as a stub, final in 13 |
| `AGENTS.md`, `.cursor/rules/` | file 02 | 01, 11 |

## 4. Release and versioning

- One product version in `kuro-rs/Cargo.toml` `[workspace.package]`; `xtask` stamps
  the shell assembly with the same value.
- SemVer. The protocol version is a separate integer that changes only on breaking
  wire changes.
- Conventional commit subjects (already in use).
- Tag `vX.Y.Z`; the release job attaches zipped binaries without models and the
  output of `cargo xtask gate full`.
- `docs/guides/release.md` holds the checklist.

## 5. README and tutorials

README target: quickstart in under 10 minutes on a machine that already has the
toolchains. Unverified: the first cold build compiles whisper.cpp and may alone exceed
that; phase 05 measures it, and prebuilt release binaries are the fallback.

Quickstart outline:
1. Requirements: Windows 10 19041 or later, x64 CPU with AVX2 (finding N17), Rust
   stable, .NET 10 SDK, LLVM (libclang), MSVC build tools.
2. "Use this template", clone.
3. `cargo xtask build`
4. `kuro init --defaults` (example profile, WinRT voice, small STT model with a
   checksum, every network feature off).
5. `kuro doctor`
6. `scripts\Start-Kuro.ps1`, hold Ctrl+Alt+Space, say "what time is it".

"Use this template" checklist (`docs/guides/use-this-template.md`): fresh history is
automatic; run `cargo xtask gen codeowners --owner <handle>`; run `kuro init` to name
the assistant; keep personal data in `%LOCALAPPDATA%\Kuro\profile` or a private repo
referenced by `KURO_PROFILE_DIR`; turn on branch protection with the `portable`,
`windows`, and `protected` checks.

`docs/guides/add-a-feature.md`, the 10 minute tutorial (written in phase 11 and
dry-run by a fresh agent): scaffold `dice`, add one exemplar, write the failing test,
implement `execute`, run the fast gate, try it with `kuro say "roll a dice"`.

Other guides: `add-a-setting.md`, `add-a-shell-page.md`, `add-an-adapter.md`,
`fix-a-routing-bug.md`, `change-a-hud-animation.md`. Each is under 80 lines and
mirrors one row of the agent-ability test.

Generated docs: `docs/features/<id>.md` per feature (intents with example phrases,
settings, secrets, tier, failure modes, health states), `docs/CONFIG.md`,
`docs/REPO_MAP.md`.

## 6. Docs drift and link checks

`cargo xtask check links`: relative links and anchors resolve; any backticked repo
path in a doc must exist (this would have caught the missing n8n probe script).
`cargo xtask gen --check`: generated docs are fresh.
Stale-term list (`kuro-rs/xtask/data/stale_terms.txt`): `XTTS`, `pyttsx3`,
`legacy/python`, outside `docs/spec/12-history.md`.

## 7. Theme instead of brand lock

The palette and font equality checks in `Test-KuroInvariants.ps1` become a validity
check in phase 17: the selected theme file parses, defines every required token, has
ink-on-canvas contrast of at least 4.5 to 1, and every referenced font file exists
with a license text beside it. The current Kuro look ships as the default theme
`Theme/themes/kuro.json` (open question 5).

## 8. Keep, rewrite, delete: documents

| File | Decision | Reason |
|------|----------|--------|
| `README.md` | rewrite | personal IP at line 5; needs the template quickstart |
| `CONTRIBUTING.md` | rewrite | 25 lines, no tutorial, mentions bench Python |
| `DESK_POWER.md` | rewrite into the desk power feature README | describes the legacy Wi-Fi watcher; em dash |
| `docs/NATIVE_REWRITE_SPEC.md` | split into `docs/spec/`, then delete | 474 lines, self-contradictory |
| `docs/HUD_DESIGN.md` | keep, fix | bar count must say 11 |
| `docs/SHELL.md` | rewrite | says "stay in Python" |
| `docs/N8N.md` | rewrite into `docs/integrations/n8n.md` | personal IP, missing script reference |
| `docs/CLOUD_INBOX.md` | rewrite into `docs/integrations/n8n-inbox.md` | personal timezone; Telegram and Groq presented as required |
| `docs/WHATSAPP.md` | rewrite into the WhatsApp feature README | must state opt-in and the terms risk |
| `docs/WIFI_WATCH.md` | delete; presence is documented from its manifest | describes a legacy script |
| `docs/PERF_BASELINE.md`, `docs/PERF_RESULTS.md` | delete | Python-era numbers on personal hardware |
| `voices/README.md` | delete | XTTS only |
| `legacy/README.md` | delete | points at removed code |
| `workflows/README.md`, `workflows/*.json`, `workflows/prompts/*` | move to `examples/n8n/cloud-inbox/`, sanitize | timezone and provider assumptions become placeholders |
| `bench/*.md`, `bench/*.json` | delete | user path `C:\Users\...`, stale results |
| `n8n_workflows.example.json` | move to `profiles/example/workflows.example.json` | belongs to the profile |
| `deploy/n8n/docker-compose.yml` | move to `examples/n8n/`, sanitize | personal IP and timezone |
| `examples/n8n/*.json` | keep | already generic |
| `.env.example` | regenerate | secrets and developer overrides only |

## 9. Keep, rewrite, delete: scripts

| File | Decision | Reason |
|------|----------|--------|
| `scripts/Test-KuroGate.ps1` | rewrite as a wrapper for `gate full` | kills and restarts Kuro today |
| `scripts/Test-KuroInvariants.ps1` | port to `xtask check`, keep a wrapper | regex checks belong in one tool; brand lock becomes theme validity |
| `scripts/Build-KuroNative.ps1` | keep as a wrapper for `xtask build` | familiar entry point |
| `scripts/Start-Kuro.ps1` | keep, shrink under 200 lines | starts from the install directory after phase 18 |
| `scripts/Install-KuroAutostart.ps1` | keep, fix | one logon task; remove the Legion task; fix the 90 s comment |
| `scripts/start_kuro_silent.vbs` | keep | avoids a console window at logon |
| `scripts/Install-GitHooks.ps1` | keep | installs pre-commit and pre-push |
| `scripts/Watch-Kuro.ps1` | delete in phase 18 | second supervisor |
| `scripts/Watch-KuroWifi.ps1` | delete | legacy; the installer already removes its task |
| `scripts/Write-KuroLegionMode.ps1` | delete (open question 7) | its only consumer is dead code |
| `scripts/Download-*.ps1` (3) | replace with `kuro doctor --fetch-models` | no checksums |
| `scripts/Measure-KuroOnnx.ps1` | delete after `xtask bench` exists | superseded |
| `scripts/bench/*.py` (12) | delete | they import deleted modules |
| `scripts/bench/gen_kokoro_vocab.py` | port to `xtask gen kokoro-vocab`, then delete | the last useful Python generator |
| `scripts/build_icons.py` | keep for now, port later | only remaining Python; low value to port |
| `.githooks/pre-commit` | rewrite | runs the fast tier |

## 10. `.gitignore` fixes

- Replace the blanket `*.png` rule with ignores for known scratch files and keep
  `KuroShell/**/Assets/**` and `assets/branding/**` tracked.
- Allow `kuro-rs/fixtures/audio/*.wav`.
- Add `.pytest_cache/`, `__pycache__/`, `.venv*/`, `Bruno/`, `snapshot.json`,
  `overlay_check.png`, `personal_denylist.local.txt`, `profiles/*/` except
  `profiles/example/`.

## 11. Personal data removal map

| Data | Where it goes |
|------|---------------|
| Tailscale and LAN IPs, hostname | removed; features report "not configured" when unset |
| "sir" | `profile.toml` `honorific`, empty in the example |
| Arrival scene and phrases | a ritual file in the owner's profile; the example ships "i'm home" |
| Song filename | `music.arrival_track` path in the owner's config |
| Mishearing table | generic part in the router data file, owner part in the profile |
| Whisper prompt | built from enabled features' `stt_bias` plus profile contacts |
| Contacts in tests | neutral names (`alex`, `sam`) |
| Mic tokens | `audio.input.prefer` and `audio.input.avoid`, generic defaults |
| Timezone, Telegram, Groq | placeholders in examples |
| Legion WMI gating | deleted, or moved to the owner's overlay |

Note for publishing: a repository created with "Use this template" starts with fresh
history, but the source repository keeps its old commits. Open question 2 decides
whether the public template is a fresh repository.
