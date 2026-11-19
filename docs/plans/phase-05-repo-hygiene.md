# Phase 05: Repo hygiene and template surface

- Size: M
- Depends on: phases 02, 03, 04 (critical fixes landed)
- Behavior change: none in the product. Adds license, notices, community files,
  CODEOWNERS, templates, README quickstart stub, personal-data scrub map.
- ADRs: 0013, 0018
- Protected paths: `.github/CODEOWNERS`, possibly `README.md`. Trailers `ADR: 0013`,
  `ADR: 0018`.

## Goal

Make the repo safe to open as a template: clear license, verified third-party
notices, community files, and no personal data in tracked files.

## Scope

In: LICENSE, THIRD_PARTY_NOTICES.md, models.lock.toml stub, CONTRIBUTING,
CODE_OF_CONDUCT, SECURITY, SUPPORT, CHANGELOG, PR/issue templates, CODEOWNERS,
dependabot, ARCHITECTURE.md stub, README quickstart rewrite, personal-data scrub
of tracked strings (Tailscale IP, hard-coded user paths, song filename already
removed in phase 04).

Out: AGENTS.md expansion (phase 06), typed config (07), scaffolders (11), full
docs/guides tutorials.

## Context card

Read:
- `docs/plans/04-repo-hygiene.md` (all sections)
- `docs/plans/02-guardrails.md` section 3 (CODEOWNERS)
- `kuro-rs/Cargo.toml` (`license = "MIT"`)
- Current `README.md`, `CONTRIBUTING.md` if present

Create:
- `LICENSE` (MIT)
- `THIRD_PARTY_NOTICES.md`
- `models.lock.toml` (schema + empty or known downloadable models)
- `CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`, `CHANGELOG.md`
- `.github/pull_request_template.md`
- `.github/ISSUE_TEMPLATE/bug.yml`, `feature.yml`
- `.github/CODEOWNERS`
- `.github/dependabot.yml`
- `ARCHITECTURE.md` stub
- `docs/guides/release.md` stub

Edit: `README.md` (quickstart), `CONTRIBUTING.md`, any tracked file with personal
IPs/paths flagged in file 04 section 6.

Do not touch: product `src/` logic except string scrub of personal data;
`KuroShell/` behavior.

Commands:
```powershell
cargo xtask gate fast
# verify no personal data: rg for known patterns from file 04
```

## Steps

1. Verify upstream licenses for models/fonts listed in file 04 before writing
   notices. Prefer SPDX identifiers.
2. Add MIT `LICENSE` matching Cargo.toml.
3. Write `THIRD_PARTY_NOTICES.md` and `models.lock.toml` with name, url placeholder,
   sha256 placeholder, license, used-by.
4. Community files per table in file 04 section 3.
5. CODEOWNERS: map from file 02 section 3 protected paths to `@owner` placeholder
   or repo owner handle.
6. Scrub personal data: Tailscale default IP, absolute user paths, names in
   comments/docs. Keep behavior via env/config keys introduced in phase 07 where a
   value must remain; for this phase replace with neutral placeholders.
7. README: requirements, clone, build, init stub note pointing to phase 08,
   link to AGENTS.md and docs/plans/README.md.
8. ARCHITECTURE.md: one-page stub linking ADRs and target tree from file 01.

## Acceptance

- LICENSE present; Cargo license field matches.
- No known personal IP/hostname/user path in tracked files (rg clean).
- PR template asks for phase id, tests-first, gate, behavior change, ADR trailer.
- Gate fast green.
- No user-visible product behavior change.

## Risks

- Wrong license claim on a model: verify before publish.
- Over-scrubbing breaks docs that need example paths: use `C:\Users\you\...` style.
