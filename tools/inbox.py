"""Cloud inbox bridge — asks n8n webhook for urgent mail / deadlines (Groq lives in n8n)."""

from __future__ import annotations

import json
import logging
import urllib.error
import urllib.request
from typing import Any

import config

logger = logging.getLogger(__name__)

NAME = "check_inbox"
DESCRIPTION = (
    "Check urgent email and deadlines via the cloud inbox agent (n8n). "
    "Use when the user asks about urgent mail, inbox, or deadlines."
)

PARAMETERS = {
    "type": "object",
    "properties": {
        "mode": {
            "type": "string",
            "description": "summary (default), deadlines, or urgent",
        }
    },
}

_TIMEOUT_SEC = 25


def run(arguments: dict[str, Any]) -> str:
    mode = str((arguments or {}).get("mode") or "summary").strip().lower()
    if mode not in ("summary", "deadlines", "urgent"):
        mode = "summary"
    return fetch_inbox(mode)


def fetch_inbox(mode: str = "summary") -> str:
    """POST to n8n; return spoken-friendly text or a real error."""
    url = (config.N8N_INBOX_WEBHOOK_URL or "").strip()
    secret = (config.N8N_WEBHOOK_SECRET or "").strip()
    if not url:
        return (
            "Cloud inbox is not configured. Set N8N_INBOX_WEBHOOK_URL in .env "
            "(see docs/CLOUD_INBOX.md)."
        )
    if not secret:
        return "Cloud inbox secret is missing. Set N8N_WEBHOOK_SECRET in .env."

    body = json.dumps({"mode": mode}).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=body,
        method="POST",
        headers={
            "Content-Type": "application/json",
            "X-Kuro-Secret": secret,
            "Accept": "application/json",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=_TIMEOUT_SEC) as resp:
            raw = resp.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as e:
        detail = ""
        try:
            detail = e.read().decode("utf-8", errors="replace")[:200]
        except Exception:
            pass
        logger.warning("Inbox webhook HTTP %s: %s", e.code, detail)
        if e.code in (401, 403):
            return "Cloud inbox rejected the secret. Check N8N_WEBHOOK_SECRET."
        return f"Cloud inbox returned HTTP {e.code}."
    except urllib.error.URLError as e:
        logger.warning("Inbox webhook unreachable: %s", e)
        return "I couldn't reach the cloud inbox. Is the n8n webhook active?"
    except TimeoutError:
        return "Cloud inbox timed out."
    except Exception:
        logger.exception("Inbox webhook failed")
        return "Cloud inbox failed unexpectedly."

    try:
        data = json.loads(raw) if raw else {}
    except json.JSONDecodeError:
        logger.warning("Inbox webhook non-JSON: %s", raw[:200])
        return "Cloud inbox returned an unexpected response."

    if not isinstance(data, dict):
        return "Cloud inbox returned an unexpected response."

    if data.get("ok") is False:
        err = str(data.get("error") or "unknown error")
        if "unauthorized" in err.lower():
            return "Cloud inbox rejected the secret. Check N8N_WEBHOOK_SECRET."
        return f"Cloud inbox error: {err}."

    spoken = (data.get("spoken") or data.get("summary") or "").strip()
    if spoken:
        # Keep TTS short
        if len(spoken) > 280:
            spoken = spoken[:277].rstrip() + "..."
        return spoken

    items = data.get("items") or []
    if isinstance(items, list) and items:
        bits = []
        for it in items[:3]:
            if not isinstance(it, dict):
                continue
            subj = str(it.get("subject") or "").strip()
            frm = str(it.get("from") or "").strip()
            if subj:
                bits.append(f"{subj}" + (f" from {frm}" if frm else ""))
        if bits:
            return "Urgent items: " + "; ".join(bits) + "."

    return "Nothing urgent in the inbox right now."
