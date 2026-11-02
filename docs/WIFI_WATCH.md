# Home Wi‑Fi watcher

[`scripts/Watch-KuroWifi.ps1`](../scripts/Watch-KuroWifi.ps1) keeps Kuro running **only on your home network**. Away from home (after a short grace), it stops Kuro so Whisper/XTTS/GPU work stop draining battery.

Cloud inbox (n8n → Telegram) is separate and keeps running in the cloud — see [CLOUD_INBOX.md](CLOUD_INBOX.md).

## Setup

1. Set home SSID(s) — one of:
   - `%LOCALAPPDATA%\Kuro\home_ssid.txt` (one name per line), or
   - env `KURO_HOME_SSID=MyWifi,MyWifi-5G`, or
   - `.\scripts\Watch-KuroWifi.ps1 -HomeSsid "MyWifi","MyWifi-5G"`
2. Install autostart (uses the watcher at logon):

   ```powershell
   .\scripts\Install-KuroAutostart.ps1 -HomeSsid "YourWifiName" -StartNow
   ```

   If you omit `-HomeSsid` while already on home Wi‑Fi, the installer writes the current SSID into `home_ssid.txt`.

## Escape hatches

| Flag | Effect |
|------|--------|
| `%LOCALAPPDATA%\Kuro\DISABLED` | Watcher idle; Start-Kuro also refuses (clear with `Start-Kuro.ps1 -ForceRestart`) |
| `%LOCALAPPDATA%\Kuro\AWAY_OK` | Never stop Kuro when off home Wi‑Fi |

## Logs

`%LOCALAPPDATA%\Kuro\wifi-watch.log`
