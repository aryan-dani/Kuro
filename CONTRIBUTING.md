# Contributing

Kuro is Windows only. The shell is WinUI 3 (`net10.0-windows10.0.19041.0`, x64). The engine is `kuro-engine.exe` in `kuro-rs/`. There is no Python runtime path.

## Build

```powershell
.\scripts\Build-KuroNative.ps1
```

## Gate

```powershell
.\scripts\Test-KuroGate.ps1
.\scripts\Install-GitHooks.ps1
```

## Autostart

Use the scheduled tasks installed by `scripts\Install-KuroAutostart.ps1`. They launch through `start_kuro_silent.vbs` so no PowerShell console appears at logon.

## Notes

- Optional measurement helpers under `scripts/bench/*.py` are offline tools only.
- Prefer `%LOCALAPPDATA%\Kuro\native\neonize-windows-amd64.dll` (or `KURO_NEONIZE_DLL`) for WhatsApp; do not rely on a `.venv`.
