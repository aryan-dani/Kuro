# xtask

Repo maintenance commands for Kuro guardrails. Invoked as `cargo xtask` from the repo
root or `kuro-rs/`.

## Commands

| Command | Purpose |
|---------|---------|
| `check size` | Line-count ratchet and README caps |
| `check text` | No emoji, em dash, or en dash in tracked text |
| `check protected --base <ref>` | Protected paths need ADR trailer + doc |
| `gate fast` | Pre-commit tier (checks, fmt, clippy, targeted tests) |
| `gate full` | Pre-push / CI tier (full workspace tests and smoke) |
| `bench [router stt tts turn]` | Local perf smoke; `router` default (no models, no daemon) |

Data files in `data/` are the single source for allowlists and protected globs.
