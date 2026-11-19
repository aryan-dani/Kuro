# Example profile

This folder is the tracked, harmless default profile for Kuro. It keeps optional
integrations off and matches engine defaults when you do not override env vars.

## Use

- Edit `config.toml` here only when you want to change the shared example for all
  clones (rare).
- For personal settings, use `profiles/local/` (gitignored) or env vars. Secrets
  belong under `%LOCALAPPDATA%\Kuro\`, not in the repo.

## Load order

1. Built-in defaults
2. `profiles/example/config.toml` when present at repo root
3. Process environment (`KURO_*` and related keys)

Later phases add user profile dirs and generated settings; this file is the safe
baseline checked into git.