# Contributing

Kuro is Windows only. The shell is WinUI 3 (`net10.0-windows10.0.19041.0`, x64). The default engine is `kuro-engine.exe` in `kuro-rs/`. `KURO_LEGACY_PYTHON=1` keeps the Python 3.11 engine in `.venv`.

## Setup

1. Create the virtualenv and install `requirements.txt`. The Whisper worker still uses that environment for the local faster-whisper weights.
2. Copy `.env.example` to `.env` and fill it in on your machine. Never commit it.
3. Build both the engine and the shell with `.\scripts\Build-KuroNative.ps1`. The script stops a running shell or engine first so `KuroShell.Core.dll` and `kuro-engine.exe` are not locked.

Do not commit `.env`, `api_token`, `credentials.json`, `token.json`, Tuya endpoint cache, timing CSVs, or model files. Those stay under `%LOCALAPPDATA%\Kuro` or in ignored files. Logs can contain private speech and message text. Leave them out of git as well.

## Tests

Run the full zero-warning gate locally:

```powershell
.\scripts\Test-KuroGate.ps1
```

Install the git pre-commit hook once per clone:

```powershell
.\scripts\Install-GitHooks.ps1
```

`Build-KuroNative.ps1` calls `Install-GitHooks.ps1` as well. The hook runs `Test-KuroGate.ps1`, which stops on the first failed stage.

Native engine only:

```powershell
cargo test --manifest-path .\kuro-rs\Cargo.toml --workspace
```

Python engine, when you are changing the legacy path:

```powershell
.\.venv\Scripts\python.exe -m pytest
```

Shell tests:

```powershell
dotnet test .\KuroShell\KuroShell.Tests\KuroShell.Tests.csproj -c Debug -p:Platform=x64
```

Set `KURO_GATE_CI=1` when you want the live smoke stage to skip the global hotkey and speech workers while still exercising the HTTP API.

## Changes

Keep the hotkey on Ctrl+Alt+Space in the engine. The bottom mark stays a small always-on-top window. Repaints must not move it. Tools should return a speakable string and should not raise. Do not add emojis to docs or UI copy. Paper `#F5F4EE`, white `#FFFFFF`, ink `#141414`, muted `#52514E`. Settings corners stay square. Titles stay Space Grotesk, labels stay JetBrains Mono, and the chat card stays Poppins.

