"""Voice-triggered n8n workflows. Registry lives outside the repo.

The phrase router re-reads the registry on every call. DESCRIPTION is built
once when this module loads, so the LLM list updates on the next Kuro restart.
"""

from __future__ import annotations

import difflib
import json
import logging
import os
import re
import time
import urllib.error
import urllib.request
from typing import Any, Callable

import config

logger = logging.getLogger(__name__)

NAME = "run_workflow"

PARAMETERS = {
    "type": "object",
    "properties": {
        "workflow": {
            "type": "string",
            "description": "Registry name or alias of the workflow to run",
        },
        "input": {
            "type": "string",
            "description": "Optional text sent to the workflow as the text field",
        },
    },
    "required": ["workflow"],
}

_SYNC_TIMEOUT = 8.0
_ASYNC_TIMEOUT = 3.0
_REPLY_LIMIT = 300
_CUTOFF = 0.75
_VERBS = {"run", "start", "trigger", "launch"}
_FILLERS = {"please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"}
_ARTICLES = {"the", "a", "an", "my", "this"}

Opener = Callable[..., Any]


def registry_path() -> str:
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return os.path.join(root, "Kuro", "n8n_workflows.json")


def normalize(text: str) -> str:
    text = (text or "").lower().strip()
    text = re.sub(r"[^a-z0-9\s]", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    return text


def load_workflows() -> list[dict[str, Any]]:
    """Return registry entries. Never raises."""
    path = registry_path()
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
    except FileNotFoundError:
        return []
    except Exception:
        logger.warning("Workflow registry unreadable")
        return []
    raw = data.get("workflows") if isinstance(data, dict) else None
    if not isinstance(raw, dict):
        return []
    found: list[dict[str, Any]] = []
    for key, spec in raw.items():
        if not isinstance(spec, dict):
            continue
        webhook = str(spec.get("webhook") or "").strip().strip("/")
        if not webhook:
            continue
        aliases = []
        for alias in spec.get("aliases") or []:
            cleaned = normalize(str(alias))
            if cleaned:
                aliases.append(cleaned)
        mode = str(spec.get("mode") or "sync").strip().lower()
        if mode not in ("sync", "async"):
            mode = "sync"
        found.append(
            {
                "id": str(key),
                "webhook": webhook,
                "description": str(spec.get("description") or "").strip(),
                "aliases": aliases,
                "mode": mode,
                "takes_input": bool(spec.get("takes_input")),
                "spoken_name": str(key).replace("_", " "),
            }
        )
    return found


def _labels(workflows: list[dict[str, Any]]) -> list[tuple[str, dict[str, Any]]]:
    pairs: list[tuple[str, dict[str, Any]]] = []
    for wf in workflows:
        pairs.append((normalize(wf["id"].replace("_", " ")), wf))
        pairs.append((normalize(wf["id"]), wf))
        for alias in wf["aliases"]:
            pairs.append((alias, wf))
    return [(label, wf) for label, wf in pairs if label]


def resolve_workflow(query: str, workflows: list[dict[str, Any]] | None = None) -> dict[str, Any] | None:
    """Exact name or alias, then difflib at 0.75. None means refuse."""
    workflows = load_workflows() if workflows is None else workflows
    query_n = normalize(query)
    if not query_n or not workflows:
        return None
    pairs = _labels(workflows)
    for label, wf in pairs:
        if query_n == label:
            return wf
    choices = []
    seen: set[str] = set()
    for label, _wf in pairs:
        if label not in seen:
            choices.append(label)
            seen.add(label)
    hits = difflib.get_close_matches(query_n, choices, n=1, cutoff=_CUTOFF)
    if not hits:
        return None
    for label, wf in pairs:
        if label == hits[0]:
            return wf
    return None


def _strip_fillers(text: str) -> str:
    toks = text.split()
    while toks and toks[0] in _FILLERS:
        toks = toks[1:]
    return " ".join(toks)


def _strip_articles(text: str) -> str:
    toks = text.split()
    while toks and toks[0] in _ARTICLES:
        toks = toks[1:]
    return " ".join(toks)


def _alias_pairs(workflows: list[dict[str, Any]]) -> list[tuple[str, dict[str, Any]]]:
    pairs = _labels(workflows)
    pairs.sort(key=lambda item: len(item[0]), reverse=True)
    return pairs


def _match_body(body: str, pairs: list[tuple[str, dict[str, Any]]]) -> tuple[dict[str, Any], str] | None:
    if not body:
        return None
    for alias, wf in pairs:
        if body == alias:
            return wf, ""
    for alias, wf in pairs:
        if wf["takes_input"] and body.startswith(alias + " "):
            return wf, body[len(alias) + 1 :].strip()
    return None


def match_phrase(text: str, workflows: list[dict[str, Any]] | None = None) -> tuple[dict[str, Any], str] | None:
    """LLM-free route. Returns (workflow, input_text) or None."""
    workflows = load_workflows() if workflows is None else workflows
    if not workflows:
        return None
    body = _strip_fillers(normalize(text))
    pairs = _alias_pairs(workflows)
    hit = _match_body(body, pairs)
    if hit:
        return hit
    toks = body.split()
    if not toks or toks[0] not in _VERBS:
        return None
    rest = " ".join(toks[1:])
    hit = _match_body(rest, pairs)
    if hit:
        return hit
    return _match_body(_strip_articles(rest), pairs)


def prompt_fragment() -> str:
    """Whisper bias text. Empty string if the registry is missing. Never raises."""
    try:
        bits: list[str] = []
        for wf in load_workflows():
            for alias in wf["aliases"]:
                if alias not in bits:
                    bits.append(alias)
        if not bits:
            return ""
        return " " + ". ".join(bits).capitalize() + "."
    except Exception:
        return ""


def _describe() -> str:
    try:
        workflows = load_workflows()
    except Exception:
        workflows = []
    if not workflows:
        return (
            "Run a named n8n workflow from the local registry. "
            "No workflows are configured yet."
        )
    bits = []
    for wf in workflows:
        alias = ", ".join(wf["aliases"]) if wf["aliases"] else wf["spoken_name"]
        desc = wf["description"] or wf["spoken_name"]
        bits.append(f"{wf['id']} (say: {alias}): {desc}")
    return "Run one configured n8n workflow. " + " ".join(bits)


DESCRIPTION = _describe()


def _configured() -> bool:
    return bool((config.KURO_N8N_BASE_URL or "").strip() and (config.KURO_N8N_TOKEN or "").strip())


def _webhook_url(webhook: str) -> str:
    base = (config.KURO_N8N_BASE_URL or "").rstrip("/")
    return f"{base}/{webhook.lstrip('/')}"


def _speak_reply(data: dict[str, Any]) -> str:
    reply = str(data.get("reply") or "").strip()
    if not reply:
        return "Success: Done, sir."
    if len(reply) > _REPLY_LIMIT:
        reply = reply[: _REPLY_LIMIT - 3].rstrip() + "..."
    return f"Success: {reply}"


def dispatch(wf: dict[str, Any], text: str, opener: Opener | None = None) -> str:
    """POST one workflow. opener is urllib.request.urlopen unless tests inject one."""
    if not _configured():
        return "n8n is not configured."
    opener = opener or urllib.request.urlopen
    mode = wf.get("mode") or "sync"
    timeout = _SYNC_TIMEOUT if mode == "sync" else _ASYNC_TIMEOUT
    payload = json.dumps({"text": text or "", "source": "kuro"}).encode("utf-8")
    req = urllib.request.Request(
        _webhook_url(str(wf["webhook"])),
        data=payload,
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json",
            "X-Kuro-Token": config.KURO_N8N_TOKEN,
        },
    )
    started = time.perf_counter()
    status: int | str = "none"
    try:
        with opener(req, timeout=timeout) as resp:
            status = getattr(resp, "status", None) or getattr(resp, "code", 200)
            raw = resp.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as e:
        status = e.code
        logger.info(
            "n8n workflow=%s mode=%s status=%s duration=%.2fs",
            wf.get("id"),
            mode,
            status,
            time.perf_counter() - started,
        )
        if e.code in (401, 403):
            return "n8n rejected my token."
        if e.code == 404:
            return "That workflow is not active in n8n."
        return f"n8n returned HTTP {e.code}."
    except TimeoutError:
        status = "timeout"
        logger.info(
            "n8n workflow=%s mode=%s status=%s duration=%.2fs",
            wf.get("id"),
            mode,
            status,
            time.perf_counter() - started,
        )
        return "n8n timed out."
    except urllib.error.URLError:
        status = "unreachable"
        logger.info(
            "n8n workflow=%s mode=%s status=%s duration=%.2fs",
            wf.get("id"),
            mode,
            status,
            time.perf_counter() - started,
        )
        return "I can't reach n8n."
    except Exception:
        logger.info(
            "n8n workflow=%s mode=%s status=error duration=%.2fs",
            wf.get("id"),
            mode,
            time.perf_counter() - started,
        )
        return "I can't reach n8n."

    logger.info(
        "n8n workflow=%s mode=%s status=%s duration=%.2fs",
        wf.get("id"),
        mode,
        status,
        time.perf_counter() - started,
    )
    if mode == "async":
        return f"Success: Started {wf.get('spoken_name') or 'workflow'}."
    try:
        data = json.loads(raw) if raw else {}
    except json.JSONDecodeError:
        return "n8n returned something I could not read."
    if not isinstance(data, dict):
        return "n8n returned something I could not read."
    return _speak_reply(data)


def run(arguments: dict[str, Any] | None) -> str:
    try:
        args = arguments or {}
        query = str(args.get("workflow") or "").strip()
        text = str(args.get("input") or "")
        if not _configured():
            return "n8n is not configured."
        workflows = load_workflows()
        if not workflows:
            return "No workflows are configured."
        wf = resolve_workflow(query, workflows)
        if wf is None:
            return "I don't know that workflow."
        return dispatch(wf, text)
    except Exception:
        logger.exception("run_workflow failed")
        return "I couldn't run that workflow."
