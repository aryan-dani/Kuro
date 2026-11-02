# Desk power layout (Kuro smart plug)

The Tuya smart plug switches the **extension board** that feeds desk lights and the monitor.

## Safe layout

- **Always-on wall outlet:** laptop charger, Wi‑Fi router / mesh (required so the plug can be commanded).
- **Smart plug → extension board:** monitor, lights, other desk accessories.

Do **not** put the router on the switched board — if Wi‑Fi dies when the board is off, voice cannot turn the desk back on. The plug’s physical button remains the manual override.

## Voice

- “Turn on/off the lights”, “monitor on/off”, “desk on/off” → plug scenes `power_on` / `power_off`
- “Good night” → board off + stop music
- “Wake up daddy’s home” → board on + song

## Cloud inbox (separate)

Email → Telegram triage runs in **n8n Cloud**, not on this board. See [docs/CLOUD_INBOX.md](docs/CLOUD_INBOX.md).

## Away from home

Autostart runs the **Wi‑Fi watcher** (`scripts/Watch-KuroWifi.ps1`): Kuro starts only on home SSID and stops when you leave, so GPU/STT/TTS do not drain battery on campus/coffee Wi‑Fi. See [docs/WIFI_WATCH.md](docs/WIFI_WATCH.md).
