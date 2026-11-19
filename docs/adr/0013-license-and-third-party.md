# ADR 0013: License and third-party terms

- Status: Accepted
- Date: 2026-11-14
- Related: finding T4; `docs/plans/04-repo-hygiene.md` sections 1 and 2; phase 05

## Context

The workspace declares `license = "MIT"` but the repository has no `LICENSE` file,
so it is legally "all rights reserved" to a visitor. A public template also inherits
obligations from what it tells users to download: speech models, the Piper
executable, espeak-ng, the neonize DLL, fonts, and LLM weights. One source file
names a copyrighted song.

## Options

1. MIT.
2. Apache-2.0.
3. Dual MIT or Apache-2.0.

## Decision

MIT, as a single root `LICENSE`. It matches the existing declaration and is the
lowest-friction choice for a template. The owner can overrule (open question 1).

Third-party policy:
- The repository distributes source code and OFL fonts only. No model, voice, or
  third-party binary is committed.
- `models.lock.toml` records url, sha256, and license for every model the tools can
  fetch. `kuro doctor --fetch-models` shows the license before downloading.
- `THIRD_PARTY_NOTICES.md` lists fonts, models, and external tools, after each entry
  has been verified against its upstream repository. Entries in this plan are from
  memory and are not yet verified.
- Components with copyleft or unclear terms are optional and off by default: Piper,
  espeak-ng (both external processes), and the WhatsApp bridge (an unofficial client
  that may breach the service terms).
- No copyrighted media is referenced by name. The arrival track is a path in the
  user's profile.
- `cargo-deny` enforces an allow-list for Rust crates.
- Any model added later (wake word, new voices) needs a recorded license check before
  it enters `models.lock.toml`.

## Consequences

- Forks carry one license text.
- No patent grant; contributors who need one can ask for a relicense discussion.
- Users see license information at the moment they download a model.

## Enforced by

`cargo deny check licenses` in the scheduled job; a check that `models.lock.toml`
entries have a non-empty `license`; the personal-data and media-name text check.
