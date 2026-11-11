![Kuro](assets/branding/kuro-logo-full.svg)

Kuro is a Windows desk voice agent. Hold Ctrl+Alt+Space, speak, and release. The native engine transcribes with the local Whisper weights, routes phrases first, and speaks with Piper. Anything the phrases do not handle can still go to the local model. `KURO_LEGACY_PYTHON=1` starts the older Python engine instead.

The desk plug and the news feed stay on the hub at `http://100.78.60.50:18200`. Mail goes out through Telegram via n8n Cloud. The hotkey is owned by `kuro-engine.exe`, not by the shell.

## Shell and engine

`kuro-engine.exe` is the voice process. It serves `127.0.0.1:8765` when `KURO_API=on`, watches home Wi-Fi, and holds Ctrl+Alt+Space. Whisper weights stay in a worker that starts in the background at launch and exits when Kuro is away from home or when `/shutdown` runs. Piper uses the ONNX voice under `bench/models/piper` through `piper.exe`. The API token stays in `%LOCALAPPDATA%\Kuro\api_token` and is never written to the repo.

`KuroShell` is the WinUI 3 window and the always-on-top mark at the bottom of the screen. When the shell is running it owns the tray and the on-screen mark, and it starts `kuro-engine.exe` with `KURO_API=on`. If that binary is missing, or `KURO_LEGACY_PYTHON=1` is set, it starts `run_kuro.py`.

## Run

Build the engine and both shell configurations:

```powershell
.\scripts\Build-KuroNative.ps1
```

Before you commit, run the full gate and install hooks once per clone:

```powershell
.\scripts\Test-KuroGate.ps1
.\scripts\Install-GitHooks.ps1
```

That script stops a running `KuroShell`, `kuro-engine`, or legacy Python Kuro so the output files are not locked. Then, from the repo root:

```powershell
.\scripts\Start-Kuro.ps1
```

To use the window and the bottom mark, start the shell instead:

```powershell
.\KuroShell\KuroShell\bin\x64\Debug\net10.0-windows10.0.19041.0\win-x64\KuroShell.exe
```

Force the Python engine:

```powershell
$env:KURO_LEGACY_PYTHON = "1"
.\scripts\Start-Kuro.ps1
```

Brand files live in `assets/branding/`. Regenerate the icon and the tray images with:

```powershell
.\.venv\Scripts\python.exe .\scripts\build_icons.py
```

## Brand

Paper `#F5F4EE`, white `#FFFFFF`, ink `#141414`, muted `#52514E`. Settings corners are square. Titles use Space Grotesk. Labels use JetBrains Mono. The chat card under the pill uses Poppins with a 22 px radius. Fonts are bundled under `KuroShell/KuroShell/Assets/Fonts` with their OFL licenses. Do not commit `.env`, `api_token`, or anything under `%LOCALAPPDATA%\Kuro`.

