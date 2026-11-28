# ADR 0022: Grouped Dependabot updates and Ubuntu protected CI build flags

- Status: Accepted
- Date: 2026-11-28
- Related: ADR 0018, ADR 0021

## Context

Two maintenance issues live under the protected `.github/` tree:

1. `.github/dependabot.yml` opens a separate pull request for every Cargo crate and NuGet package update, creating a large backlog of stale PRs whenever `main` advances.
2. The `protected` job in `.github/workflows/ci.yml` runs `cargo xtask check protected` on `ubuntu-latest` without overriding the Windows MSVC `CFLAGS`/`CXXFLAGS` from `kuro-rs/.cargo/config.toml` and without installing `libasound2-dev` (required transitively by `xtask` via `kuro-router -> kuro-tools -> rodio -> cpal -> alsa-sys`). Because `protected` only runs on `pull_request`, pushes to `main` stay green while every PR fails `protected` on Linux.

## Decision

1. Group weekly updates in `.github/dependabot.yml` with one group per ecosystem (`cargo` and `nuget`).
2. Set `CFLAGS: "-O2"` and `CXXFLAGS: "-O2"` and install `libasound2-dev` in the `protected` job in `.github/workflows/ci.yml`, matching the `portable` job.

## Consequences

- Weekly Cargo and NuGet updates are consolidated into one grouped pull request per ecosystem instead of one PR per package.
- The `protected` CI job builds and runs `cargo xtask check protected` cleanly on `ubuntu-latest` for pull requests.

## Enforced by

`cargo xtask check protected` (local gate and `protected` CI job) and GitHub Dependabot config validation.
