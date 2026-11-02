"""HTTP + WebSocket client for the Kuro hub.

STT/TTS/music stay local. Desk plug, news, and rituals go to the hub.
"""

from __future__ import annotations

import json
import logging
import threading
import time
from typing import Any, Callable

import config

logger = logging.getLogger(__name__)

_seen_events: set[str] = set()
_seen_lock = threading.Lock()
_ws_stop = threading.Event()
_ws_thread: threading.Thread | None = None


def configured() -> bool:
    return bool((config.KURO_HUB_URL or "").strip() and (config.KURO_HUB_TOKEN or "").strip())


def _headers() -> dict[str, str]:
    return {
        "Authorization": f"Bearer {config.KURO_HUB_TOKEN}",
        "Content-Type": "application/json",
    }


def command(text: str, *, early: bool = False, source: str = "laptop", timeout: float = 8.0) -> dict[str, Any] | None:
    """POST /v1/command. Returns None if the hub is unreachable."""
    if not configured():
        return None
    import urllib.error
    import urllib.request

    url = config.KURO_HUB_URL.rstrip("/") + "/v1/command"
    body = json.dumps({"text": text, "source": source, "early": early}).encode("utf-8")
    req = urllib.request.Request(url, data=body, headers=_headers(), method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        logger.warning("Hub command HTTP %s", e.code)
        return None
    except Exception as e:
        logger.warning("Hub unreachable: %s", e)
        return None


def list_tools(timeout: float = 5.0) -> list[dict]:
    if not configured():
        return []
    import urllib.error
    import urllib.request

    url = config.KURO_HUB_URL.rstrip("/") + "/v1/tools"
    req = urllib.request.Request(url, headers=_headers(), method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            return data if isinstance(data, list) else []
    except Exception as e:
        logger.warning("Hub tools fetch failed: %s", e)
        return []


def execute_tool(name: str, arguments: dict | None = None, timeout: float = 30.0) -> str:
    if not configured():
        return "Hub is not configured."
    import urllib.error
    import urllib.request

    url = config.KURO_HUB_URL.rstrip("/") + f"/v1/tools/{name}"
    body = json.dumps({"arguments": arguments or {}}).encode("utf-8")
    req = urllib.request.Request(url, data=body, headers=_headers(), method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            return str(data.get("result", ""))
    except Exception as e:
        logger.warning("Hub tool %s failed: %s", name, e)
        return f"Hub tool failed: {e}"


def apply_client_actions(actions: list[dict[str, Any]] | None) -> None:
    if not actions:
        return
    import music

    for action in actions:
        if not isinstance(action, dict):
            continue
        if action.get("type") != "music":
            continue
        op = (action.get("op") or "").lower()
        try:
            if op == "pause":
                music.pause_song()
            elif op == "resume":
                music.resume_song()
            elif op == "stop":
                music.stop_song()
            elif op == "play" and action.get("track") == "daddy_home":
                music.play_should_i_stay_or_should_i_go()
            else:
                logger.warning("Unknown music action: %s", action)
        except Exception:
            logger.exception("client_action failed: %s", action)


def _ack(ws, event_id: str, ok: bool, detail: str = "") -> None:
    try:
        ws.send(
            json.dumps(
                {
                    "type": "action_result",
                    "event_id": event_id,
                    "ok": ok,
                    "detail": detail,
                }
            )
        )
    except Exception:
        logger.debug("WS ack failed", exc_info=True)


def _handle_event(event: dict[str, Any], speak: Callable[[str], None] | None, notify: Callable[[str], None] | None, ws) -> None:
    event_id = str(event.get("id") or "")
    if event_id:
        with _seen_lock:
            if event_id in _seen_events:
                return
            _seen_events.add(event_id)
            if len(_seen_events) > 500:
                _seen_events.clear()

    etype = event.get("type")
    payload = event.get("payload") or {}
    ok = True
    detail = ""
    try:
        if etype == "speak":
            text = str(payload.get("text") or "")
            if text and speak:
                speak(text)
        elif etype == "notify":
            text = str(payload.get("text") or "")
            if text and notify:
                notify(text)
            elif text:
                logger.info("Hub notify: %s", text)
        elif etype == "client_action":
            apply_client_actions([payload])
        else:
            detail = f"unknown type {etype}"
            ok = False
    except Exception as e:
        ok = False
        detail = str(e)
        logger.exception("Event handler error")
    if event_id and ws is not None:
        _ack(ws, event_id, ok, detail)


def _ws_loop(speak: Callable[[str], None] | None, notify: Callable[[str], None] | None) -> None:
    try:
        import websocket  # type: ignore
    except ImportError:
        logger.warning("websocket-client not installed; hub event stream disabled")
        return

    base = config.KURO_HUB_URL.rstrip("/")
    if base.startswith("https://"):
        ws_url = "wss://" + base[len("https://") :] + "/v1/events"
    elif base.startswith("http://"):
        ws_url = "ws://" + base[len("http://") :] + "/v1/events"
    else:
        ws_url = base + "/v1/events"

    backoff = 1.0
    while not _ws_stop.is_set():
        try:
            logger.info("Connecting hub events WS")
            ws = websocket.create_connection(
                ws_url,
                header=[f"Authorization: Bearer {config.KURO_HUB_TOKEN}"],
                timeout=15,
            )
            backoff = 1.0
            ws.settimeout(30)
            while not _ws_stop.is_set():
                try:
                    raw = ws.recv()
                except Exception:
                    break
                if not raw:
                    break
                try:
                    event = json.loads(raw)
                except Exception:
                    continue
                if isinstance(event, dict) and event.get("type") == "pong":
                    continue
                if isinstance(event, dict):
                    _handle_event(event, speak, notify, ws)
            try:
                ws.close()
            except Exception:
                pass
        except Exception as e:
            logger.warning("Hub WS disconnected: %s", e)
        if _ws_stop.wait(backoff):
            break
        backoff = min(backoff * 2, 30.0)


def start_event_worker(
    speak: Callable[[str], None] | None = None,
    notify: Callable[[str], None] | None = None,
) -> None:
    global _ws_thread
    if not configured():
        logger.info("Hub not configured — event worker skipped")
        return
    if _ws_thread and _ws_thread.is_alive():
        return
    _ws_stop.clear()
    _ws_thread = threading.Thread(
        target=_ws_loop,
        args=(speak, notify),
        daemon=True,
        name="KuroHubEvents",
    )
    _ws_thread.start()


def stop_event_worker() -> None:
    _ws_stop.set()


def health(timeout: float = 3.0) -> bool:
    if not configured():
        return False
    import urllib.request

    url = config.KURO_HUB_URL.rstrip("/") + "/health"
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            return resp.status == 200
    except Exception:
        return False
