# Contributing

Kuro is Windows only. The shell is WinUI 3 (`net10.0-windows10.0.19041.0`, x64). The engine is Python 3.11 in `.venv`.

## Setup

1. Create the virtualenv and install `requirements.txt`.
2. Copy `.env.example` to `.env` and fill it in on your machine.
3. Build the shell with `dotnet build .\KuroShell\KuroShell\KuroShell.csproj -c Debug -p:Platform=x64`.

Do not commit `.env`, `api_token`, `credentials.json`, `token.json`, Tuya endpoint cache, or timing CSVs. Those stay under `%LOCALAPPDATA%\Kuro` or in ignored files. Logs can contain private speech and message text. Leave them out of git as well.

## Tests

Engine tests:

```powershell
.\.venv\Scripts\python.exe -m pytest
```

Shell tests:

```powershell
dotnet test .\KuroShell\KuroShell.Tests\KuroShell.Tests.csproj -c Debug -p:Platform=x64
```

## Changes

Keep the hotkey on Ctrl+Alt+Space in the Python listener. The bottom mark stays a small always-on-top window. Repaints must not move it. Tools should return a speakable string and should not raise. Do not add emojis to docs or UI copy.
