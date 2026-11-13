![Kuro](assets/branding/kuro-logo-full.svg)

Kuro is a Windows desk voice agent. Hold Ctrl+Alt+Space, speak, and release. The native engine transcribes with local Whisper weights, routes phrases first, and speaks with Kokoro/Piper. Anything the phrases do not handle can still go to the local Ollama model.

The desk plug and the news feed stay on the hub at `http://100.78.60.50:18200`. Mail goes out through Telegram via n8n Cloud. The hotkey is owned by `kuro-engine.exe`, not by the shell.

## Shell and engine

`kuro-engine.exe` is the voice process. It serves `127.0.0.1:8765` when `KURO_API=on`, watches home Wi-Fi, and holds Ctrl+Alt+Space. Whisper weights stay in a worker that starts in the background at launch and exits when Kuro is away from home or when `/shutdown` runs.

`KuroShell` is the WinUI 3 tray + always-on-top mark. When the shell is running it owns the tray and the on-screen mark, and it starts `kuro-engine.exe` with `KURO_API=on`.

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

Then, from the repo root:

```powershell
.\scripts\Start-Kuro.ps1
```

Silent logon autostart (no console windows):

```powershell
.\scripts\Install-KuroAutostart.ps1 -StartNow
```

Brand files live in `assets/branding/`. Regenerate tray icons with:

```powershell
python .\scripts\build_icons.py
```

## Brand

Paper `#F5F4EE`, white `#FFFFFF`, ink `#141414`, muted `#52514E`. Settings corners are square. Titles use Space Grotesk. Labels use JetBrains Mono. The chat card under the pill uses Poppins with a 22 px radius. Fonts are bundled under `KuroShell/KuroShell/Assets/Fonts` with their OFL licenses. Do not commit `.env`, `api_token`, or anything under `%LOCALAPPDATA%\Kuro`.
