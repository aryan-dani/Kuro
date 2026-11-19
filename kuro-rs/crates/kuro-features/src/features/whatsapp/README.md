# WhatsApp

WhatsApp voice feature. Phrase matching and `send_whatsapp` actuation stay in `kuro-tools`;
this folder owns speakable replies and the `WhatsappControl` port.

`default_enabled = true` matches today's behavior. Example profile `[whatsapp] enabled` still
gates legacy engine tooling until feature flags wire through.