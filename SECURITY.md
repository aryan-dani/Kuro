# Security

## Reporting

Report vulnerabilities privately through GitHub Security Advisories on this
repository. Do not open a public issue for token leaks, auth bypasses, or
remote code risks.

## Scope

In scope:

- Localhost API auth (`X-Kuro-Token`) and token generation
- Secrets handling under `%LOCALAPPDATA%\Kuro` and ignored profile files
- IPC and named-pipe auth
- Dependency issues that affect the engine or shell

Out of scope:

- Social engineering
- Issues that require physical access to an unlocked machine
- Bugs in optional third-party tools the user installs (Piper, neonize, n8n)

There is no bug bounty at this time.

## Safe defaults

- API binds to `127.0.0.1` only
- `/say` defaults to dry-run; irreversible tools require opt-in and policy
- Optional adapters (WhatsApp, Meet, hub) stay off in the example profile
