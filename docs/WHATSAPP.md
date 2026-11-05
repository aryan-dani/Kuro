# WhatsApp from voice

This sends through a linked device in the background. The WhatsApp window does not open. It does not use the WhatsApp Cloud API, and it does not replace Telegram mail in [CLOUD_INBOX.md](CLOUD_INBOX.md).

## Link it once

On first launch Kuro opens a small window titled "Link Kuro". On your phone open WhatsApp, then Linked devices, then Link a device, and scan that code. The window closes when the link succeeds. The session stays in `%LOCALAPPDATA%\Kuro\wa_session.db`. After that, sends stay invisible.

## Contacts

Create `%LOCALAPPDATA%\Kuro\contacts.json`. Names are whatever you say. Values are digits only, country code included, no plus sign:

```json
{
  "siya": "91XXXXXXXXXX"
}
```

A name that is not in the file, or only a weak fuzzy match, is refused. Kuro will not pick a different person.

## What you can say

- "Send this link to Siya on WhatsApp" copies the focused browser tab (Ctrl+L, Ctrl+C) and sends that URL.
- "Send the meeting link to Siya on WhatsApp" does the same.
- "Send my clipboard to Siya on WhatsApp" sends clipboard text.
- "WhatsApp Siya this link" sends the browser URL.

## Limits

The laptop must be awake. The phone must stay linked under Linked devices. The browser URL path only works when the focused window is a browser. A synthetic Esc is not used, so copying the address bar cannot cancel the command or start the hotkey.

## Check by hand

1. Put your own number in the contacts file and send a clipboard URL to yourself.
2. Focus a Meet tab and send that link to yourself.
3. Press Esc during the cancel window and confirm nothing sends and the HUD returns to ready.
4. Say a name that is not in the file and confirm Kuro refuses.
5. Hold Ctrl+Alt+Space after each try and confirm it is not stuck busy.
