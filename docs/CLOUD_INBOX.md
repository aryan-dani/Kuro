# Kuro Cloud Inbox

Always-on mail triage lives in **n8n Cloud** (not on the laptop). Alerts go to **Telegram**. Kuro on the PC can later ask n8n “anything urgent?” via a webhook.

```
Gmail → n8n Cloud → Groq → Telegram Bot
                ↑
         Kuro (optional webhook)
```

Laptop sleep does **not** stop alerts. Desk lights/music still need Kuro + Wi‑Fi (see [DESK_POWER.md](../DESK_POWER.md)).

WhatsApp Cloud API was dropped for this setup (Meta business verification / locked WABA). Telegram Bot API is enough for personal pings.

---

## Credential setup checklist

### 1. Telegram bot (delivery)

1. In Telegram, open [@BotFather](https://t.me/BotFather) → `/newbot` → follow prompts → copy the **bot token** (`123456:AA...`).
2. Open a chat with **your new bot** and press **Start** (or send `/start`).
3. Get your **chat id**:
   - Open `https://api.telegram.org/bot<TOKEN>/getUpdates` in a browser after messaging the bot, or
   - Message [@userinfobot](https://t.me/userinfobot) for your user id.
4. Save for n8n variables:
   - `TELEGRAM_BOT_TOKEN`
   - `TELEGRAM_CHAT_ID` (your user id, e.g. `123456789`)

No business verification. No message templates.

### 2. Groq

1. API key at [console.groq.com](https://console.groq.com).
2. In n8n: Credentials → **Header Auth**  
   - Name: `Authorization`  
   - Value: `Bearer gsk_...`
3. Model in stubs: `llama-3.3-70b-versatile`.

### 3. n8n Cloud

1. Sign up at [n8n.io](https://n8n.io) → Cloud workspace.
2. **Variables**:

   | Variable | Example |
   |----------|---------|
   | `TELEGRAM_BOT_TOKEN` | from BotFather |
   | `TELEGRAM_CHAT_ID` | your numeric chat id |
   | `KURO_WEBHOOK_SECRET` | long random string (same as Kuro `.env`) |
   | `KURO_GROQ_CLASSIFY_PROMPT` | paste from `workflows/prompts/groq_classify.txt` |
   | `KURO_GROQ_DIGEST_PROMPT` | paste from `workflows/prompts/groq_digest.txt` |

3. Import JSON from [`workflows/`](../workflows/): Urgent Mail, Daily Digest, Inbox Webhook.
4. Attach **Gmail OAuth** and **Groq Header Auth** on the relevant nodes.
5. Telegram sends use HTTP Request to `api.telegram.org` (token in URL from env — or n8n’s built-in Telegram credential).
6. Digest workflow timezone: **Asia/Kolkata**.
7. Activate Urgent Mail after a manual test send works.

### 4. Gmail OAuth (n8n)

1. Create **Gmail OAuth2** credential → connect personal Google account.
2. Scopes: read/search mail.
3. Filters already skip Promotions/Social where noted in workflow `q`.

### 5. Wire Kuro (laptop) — after cloud works

1. Production webhook URL from **Kuro Inbox Webhook** → `N8N_INBOX_WEBHOOK_URL` in `.env`.
2. `N8N_WEBHOOK_SECRET` = same as n8n `KURO_WEBHOOK_SECRET`.
3. `Start-Kuro.ps1 -ForceRestart`.
4. Voice: “any urgent mail”, “inbox check”, “deadlines today”.

---

## Quick Telegram smoke test

In PowerShell (replace token/chat id):

```powershell
$token = "YOUR_BOT_TOKEN"
$chat = "YOUR_CHAT_ID"
Invoke-RestMethod -Method Post `
  -Uri "https://api.telegram.org/bot$token/sendMessage" `
  -ContentType "application/json" `
  -Body (@{ chat_id = $chat; text = "Kuro test ping" } | ConvertTo-Json)
```

You should see **Kuro test ping** in the bot chat.

---

## Verify end-to-end

1. Email yourself with a clear deadline → Telegram within ~1–2 minutes.
2. Manually run Daily Digest → one Telegram message (or skip if empty).
3. Kuro PTT “inbox check” → spoken summary (after webhook wired).

## Ops

- Urgent workflow rate-limits to **6 alerts/hour**.
- Medium priority → digest only.
- Never commit bot tokens or `.env`.
- Keep n8n execution data retention short.
