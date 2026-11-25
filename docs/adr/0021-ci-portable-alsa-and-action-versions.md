# ADR 0021: CI portable ALSA package and GitHub Actions versions

- Status: Accepted
- Date: 2026-11-26

## Context

The `Kuro Gate` workflow in `.github/workflows/ci.yml` had two CI-only issues:

1. The `portable` job on `ubuntu-latest` failed to build `alsa-sys` (pulled in by `cpal` in the workspace) because `libasound2-dev` was not installed on the runner.
2. `actions/checkout@v4` and `actions/setup-dotnet@v4` targeted Node.js 20 and triggered Node 20 deprecation warnings on GitHub Actions.

## Decision

1. Install `libasound2-dev` in the `portable` job before running `cargo xtask` and portable crate tests.
2. Bump `actions/checkout` to `v7` and `actions/setup-dotnet` to `v6` in `.github/workflows/ci.yml`.

## Consequences

- `portable` builds and tests cleanly on `ubuntu-latest`.
- GitHub Actions runs on current Node.js versions without Node 20 deprecation warnings.
