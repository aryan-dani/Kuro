# Kuro n8n workflows

Import into **n8n Cloud** (Workflows → Import from File). Delivery is **Telegram** (not WhatsApp).

| File | Purpose |
|------|---------|
| `kuro_urgent_mail.json` | New Gmail → Groq → Telegram if high/deadline |
| `kuro_daily_digest.json` | 20:00 IST digest → Telegram |
| `kuro_inbox_webhook.json` | Webhook for laptop Kuro voice queries |
| `prompts/` | Groq system prompts |

Setup: [docs/CLOUD_INBOX.md](../docs/CLOUD_INBOX.md)

**n8n variables:** `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`, `KURO_WEBHOOK_SECRET`, Groq prompts.
