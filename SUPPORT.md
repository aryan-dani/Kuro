# Support

## Ask for help

- Open a GitHub Discussion or issue using the bug / feature templates.
- Include `cargo run -p kuro-cli -- doctor --json` output (paste the JSON block;
  redact paths if needed). On Windows you can run `kuro doctor --json` after
  installing the `kuro` binary from a release build.

### Doctor JSON shape

Top-level fields:

| Field | Type | Meaning |
|-------|------|---------|
| `ok` | bool | `true` when no check has `fail` status |
| `checks` | array | One object per diagnostic |

Each check object:

| Field | Type | Meaning |
|-------|------|---------|
| `id` | string | Stable id: `toolchain`, `profile`, `models_dir`, `api_token`, `api_bind`, `winrt_voice`, `mic_permission` |
| `status` | string | `ok`, `warn`, `fail`, or `skip` |
| `message` | string | Human-readable detail for logs and issues |

Checks that need WinRT or Windows mic settings use `skip` on non-Windows CI hosts.

## What to include

- Steps to reproduce
- Whether the golden corpus or gate failed (`cargo xtask gate fast`)
- Logs from `%LOCALAPPDATA%\Kuro` with tokens and secrets redacted

## Not support channels

Do not paste API tokens, Tuya keys, OAuth refresh tokens, or full contact lists
into public issues.
