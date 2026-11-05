# Kuro shell

The WinUI shell is `KuroShell/`. It does not own speech recognition, speech synthesis, the phrase router, or the Ctrl+Alt+Space hotkey. Those stay in Python. `scripts/Start-Kuro.ps1` still starts the engine with the shell closed.

Debug builds are unpackaged. Release builds are MSIX and register a StartupTask named `KuroShellStartup`. Settings use a solid dark surface. The HUD is a tight window around the capsule. Per-pixel desktop transparency is left off, because this Windows App SDK line does not support system backdrops for XAML windows.

The shell listens only through `http://127.0.0.1:8765` with the token in `%LOCALAPPDATA%\Kuro\api_token`. The engine binds that API only when `KURO_API=on`.

## Manual checklist

1. Install the .NET 10 SDK and the Visual Studio workload for Windows application development. Confirm `dotnet --version` prints a 10.x SDK.
2. From `KuroShell/`, run `dotnet test KuroShell.sln -c Debug` and `dotnet build KuroShell/KuroShell.csproj -c Debug`.
3. Start the unpackaged shell. If Kuro is already running, the tray should say attached and must not start a second engine.
4. With the engine stopped, start the shell and confirm a hold of Ctrl+Alt+Space still records. The Python log should show the hotkey, not a C# hook.
5. Open Settings from a left click. Check General, Voice, Devices (plug id masked), Workflows, Contacts (numbers masked), Logs, Performance, and the HUD playground. Force listening, thinking, speaking, result, error, and exit.
6. Pause listening, hold the hotkey, and confirm the engine ignores it. Resume and confirm the next hold records.
7. Quit and choose "Leave it". The tray closes and the engine keeps running. Start the shell again and confirm it attaches.
8. Quit and choose "Stop engine". The engine process exits. `scripts/Start-Kuro.ps1` can start it again with the shell closed.

A Debug build is unpackaged. A Release build writes an unsigned MSIX under `KuroShell/KuroShell/AppPackages/`. That folder is gitignored. This session installed the .NET 10 SDK, built both configurations, and ran the unit tests. It did not walk the checklist on a second monitor, at 150 percent DPI, or through an install and uninstall. Those steps are still yours.
