"""Optional localhost API for the WinUI shell. Off unless KURO_API=on.

Binds to 127.0.0.1 only. Refuses to listen when the token file is missing.
Request bodies and the token are never logged.
"""

import hmac
import json
import logging
import os
import secrets
import threading
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

_HOST = "127.0.0.1"
_DEFAULT_PORT = 8765
_paused = False
_pause_lock = threading.Lock()
_server = None
_shutdown_handler = None


def api_requested() -> bool:
    return os.getenv("KURO_API", "").strip().lower() in ("on", "1", "true", "yes")


def token_path() -> Path:
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return Path(root) / "Kuro" / "api_token"


def port() -> int:
    raw = os.getenv("KURO_API_PORT", str(_DEFAULT_PORT)).strip()
    try:
        value = int(raw)
    except ValueError:
        return _DEFAULT_PORT
    return value if 1 <= value <= 65535 else _DEFAULT_PORT


def ensure_token() -> Path:
    """Create the token file once. Does not log the value."""
    path = token_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.is_file():
        path.write_text(secrets.token_hex(32), encoding="utf-8")
    return path


def load_token() -> str:
    path = token_path()
    try:
        return path.read_text(encoding="utf-8").strip()
    except OSError:
        return ""


def token_matches(presented: str) -> bool:
    expected = load_token()
    if not expected or not presented:
        return False
    return hmac.compare_digest(presented, expected)


def is_paused() -> bool:
    with _pause_lock:
        return _paused


def on_shutdown(handler) -> None:
    global _shutdown_handler
    _shutdown_handler = handler


def set_paused(value: bool) -> None:
    global _paused
    with _pause_lock:
        _paused = bool(value)


def hud_show_text() -> bool:
    return os.getenv("KURO_HUD_SHOW_TEXT", "").strip().lower() in ("1", "true", "yes", "on")


def set_hud_show_text(value: bool) -> None:
    os.environ["KURO_HUD_SHOW_TEXT"] = "1" if value else "0"


_LLM = {"off", "fallback", "always"}
_STT = {"auto", "cuda", "npu", "cpu"}
_TTS = {"auto", "xtts", "piper", "pyttsx3"}


def _mask_number(raw: str) -> str:
    digits = "".join(ch for ch in raw if ch.isdigit())
    if len(digits) < 4:
        return "masked"
    return ("x" * (len(digits) - 2)) + digits[-2:]


def _mask_id(raw: str) -> str:
    text = (raw or "").strip()
    if len(text) <= 4:
        return "xxxx" if text else ""
    return text[:2] + ("x" * (len(text) - 4)) + text[-2:]


def public_status() -> dict[str, Any]:
    snap = {"state": "hidden", "levels": [0.0] * 11}
    try:
        import overlay

        manager = overlay.current()
        if manager is not None:
            snap = manager.snapshot()
    except Exception:
        logger.warning("HUD snapshot unavailable")
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return {
        "ok": True,
        "paused": is_paused(),
        "state": snap.get("state", "hidden"),
        "api": True,
        "logs": str(Path(root) / "Kuro"),
    }


def public_config() -> dict[str, Any]:
    import config

    return {
        "llm_mode": config.KURO_LLM_MODE,
        "stt_backend": config.KURO_STT_BACKEND,
        "tts_engine": config.KURO_TTS_ENGINE,
        "xtts_language": config.XTTS_LANGUAGE,
        "paused": is_paused(),
        "hud_show_text": hud_show_text(),
        "plug_id_masked": _mask_id(config.TUYA_DEVICE_ID),
    }


def apply_config(payload: dict[str, Any]) -> None:
    """Apply a whitelist. Raises ValueError on an unknown or bad field."""
    import config

    allowed = {
        "llm_mode",
        "stt_backend",
        "tts_engine",
        "xtts_language",
        "paused",
        "hud_show_text",
    }
    extra = set(payload) - allowed
    if extra:
        raise ValueError("unknown config field")
    if "llm_mode" in payload:
        value = str(payload["llm_mode"]).strip().lower()
        if value not in _LLM:
            raise ValueError("bad llm_mode")
        config.KURO_LLM_MODE = value
    if "stt_backend" in payload:
        value = str(payload["stt_backend"]).strip().lower()
        if value not in _STT:
            raise ValueError("bad stt_backend")
        config.KURO_STT_BACKEND = value
    if "tts_engine" in payload:
        value = str(payload["tts_engine"]).strip().lower()
        if value not in _TTS:
            raise ValueError("bad tts_engine")
        config.KURO_TTS_ENGINE = value
    if "xtts_language" in payload:
        value = str(payload["xtts_language"]).strip().lower()
        if not value.isalpha() or not 2 <= len(value) <= 8:
            raise ValueError("bad xtts_language")
        config.XTTS_LANGUAGE = value
    if "paused" in payload:
        if not isinstance(payload["paused"], bool):
            raise ValueError("bad paused")
        set_paused(payload["paused"])
    if "hud_show_text" in payload:
        if not isinstance(payload["hud_show_text"], bool):
            raise ValueError("bad hud_show_text")
        set_hud_show_text(payload["hud_show_text"])


def atomic_write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(tmp, path)


def _contacts_ok(data: Any) -> bool:
    if not isinstance(data, dict) or not data:
        return False
    for key, value in data.items():
        if not isinstance(key, str) or not key.strip():
            return False
        if not isinstance(value, str):
            return False
        digits = "".join(ch for ch in value if ch.isdigit())
        if not 8 <= len(digits) <= 15:
            return False
    return True


def _workflows_ok(data: Any) -> bool:
    if not isinstance(data, dict):
        return False
    raw = data.get("workflows")
    if not isinstance(raw, dict):
        return False
    for key, spec in raw.items():
        if not isinstance(key, str) or not isinstance(spec, dict):
            return False
        webhook = spec.get("webhook", "")
        if not isinstance(webhook, str) or "://" in webhook:
            return False
    return True


def masked_contacts(data: dict[str, Any]) -> dict[str, str]:
    out: dict[str, str] = {}
    for key, value in data.items():
        out[str(key)] = _mask_number(str(value))
    return out


def read_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def event_frame(*, include_text: bool) -> dict[str, Any]:
    snap = {"state": "hidden", "levels": [0.0] * 11}
    you = ""
    kuro = ""
    try:
        import overlay

        manager = overlay.current()
        if manager is not None:
            snap = manager.snapshot()
            if include_text:
                you, kuro = manager.transcript_pair()
    except Exception:
        logger.warning("HUD event frame unavailable")
    frame: dict[str, Any] = {
        "state": snap.get("state", "hidden"),
        "levels": snap.get("levels", [0.0] * 11),
        "paused": is_paused(),
    }
    if include_text:
        frame["you"] = you
        frame["kuro"] = kuro
    return frame


def recent_timing(limit: int = 20) -> list[dict[str, Any]]:
    import timing

    path = timing.csv_path()
    if not path.is_file():
        return []
    try:
        import csv

        with path.open(encoding="utf-8", newline="") as handle:
            rows = list(csv.DictReader(handle))
    except OSError:
        return []
    kept = []
    for row in rows[-limit:]:
        kept.append(
            {
                "kind": row.get("kind", ""),
                "stage": row.get("stage", ""),
                "ms": row.get("ms", ""),
            }
        )
    return kept


def say_text(text: str) -> dict[str, Any]:
    import phrases

    result = phrases.try_handle(text)
    if result is None:
        return {"matched": False, "reply": ""}
    reply = result.reply or ""
    if reply and not result.skip_tts:
        import tts

        tts.speak(reply, fast=bool(result.force_fast_tts))
    if result.after_speak is not None:
        result.after_speak()
    return {"matched": True, "reply": reply}


def build_app():
    from fastapi import FastAPI, HTTPException, Request, WebSocket
    from fastapi.responses import JSONResponse

    app = FastAPI(docs_url=None, redoc_url=None, openapi_url=None)

    def _require(request: Request) -> None:
        presented = request.headers.get("x-kuro-token", "")
        if not token_matches(presented):
            raise HTTPException(status_code=401, detail="unauthorized")

    @app.middleware("http")
    async def _quiet(request: Request, call_next):
        return await call_next(request)

    @app.get("/status")
    def status(request: Request):
        _require(request)
        return public_status()

    @app.get("/config")
    def get_config(request: Request):
        _require(request)
        return public_config()

    @app.put("/config")
    async def put_config(request: Request):
        _require(request)
        try:
            payload = await request.json()
        except Exception:
            return JSONResponse({"ok": False}, status_code=400)
        if not isinstance(payload, dict):
            return JSONResponse({"ok": False}, status_code=400)
        try:
            apply_config(payload)
        except ValueError:
            return JSONResponse({"ok": False}, status_code=400)
        return public_config()

    @app.get("/workflows")
    def get_workflows(request: Request):
        _require(request)
        from tools.n8n import registry_path

        path = Path(registry_path())
        if not path.is_file():
            return {"workflows": {}}
        try:
            data = read_json(path)
        except Exception:
            return JSONResponse({"ok": False}, status_code=400)
        return data if isinstance(data, dict) else {"workflows": {}}

    @app.put("/workflows")
    async def put_workflows(request: Request):
        _require(request)
        try:
            payload = await request.json()
        except Exception:
            return JSONResponse({"ok": False}, status_code=400)
        if not _workflows_ok(payload):
            return JSONResponse({"ok": False}, status_code=400)
        from tools.n8n import registry_path

        atomic_write(Path(registry_path()), json.dumps(payload, indent=2) + "\n")
        return {"ok": True}

    @app.get("/contacts")
    def get_contacts(request: Request):
        _require(request)
        from tools.whatsapp import contacts_path

        path = Path(contacts_path())
        if not path.is_file():
            return {"contacts": {}}
        try:
            data = read_json(path)
        except Exception:
            return JSONResponse({"ok": False}, status_code=400)
        if not isinstance(data, dict):
            return {"contacts": {}}
        return {"contacts": masked_contacts(data)}

    @app.put("/contacts")
    async def put_contacts(request: Request):
        _require(request)
        try:
            payload = await request.json()
        except Exception:
            return JSONResponse({"ok": False}, status_code=400)
        if not isinstance(payload, dict):
            return JSONResponse({"ok": False}, status_code=400)
        from tools.whatsapp import contacts_path

        path = Path(contacts_path())
        existing: dict[str, Any] = {}
        if path.is_file():
            try:
                loaded = read_json(path)
            except Exception:
                return JSONResponse({"ok": False}, status_code=400)
            if isinstance(loaded, dict):
                existing = loaded
        merged: dict[str, Any] = dict(existing)
        for key, value in payload.items():
            if value == "":
                merged.pop(key, None)
            else:
                merged[key] = value
        if not _contacts_ok(merged):
            return JSONResponse({"ok": False}, status_code=400)
        atomic_write(path, json.dumps(merged, indent=2) + "\n")
        return {"ok": True}

    @app.get("/timing")
    def get_timing(request: Request):
        _require(request)
        return {"rows": recent_timing(20)}

    @app.post("/say")
    async def say(request: Request):
        _require(request)
        try:
            payload = await request.json()
        except Exception:
            return JSONResponse({"ok": False}, status_code=400)
        if not isinstance(payload, dict):
            return JSONResponse({"ok": False}, status_code=400)
        text = payload.get("text")
        if not isinstance(text, str) or not text.strip():
            return JSONResponse({"ok": False}, status_code=400)
        return say_text(text.strip())

    @app.post("/shutdown")
    def shutdown(request: Request):
        _require(request)
        handler = _shutdown_handler
        if handler is not None:
            threading.Thread(target=handler, name="KuroApiShutdown", daemon=True).start()
        return {"ok": True}

    @app.websocket("/events")
    async def events(socket: WebSocket):
        import asyncio

        presented = socket.headers.get("x-kuro-token", "")
        if not token_matches(presented):
            await socket.close(code=1008)
            return
        await socket.accept()
        try:
            while True:
                await socket.send_json(event_frame(include_text=hud_show_text()))
                await asyncio.sleep(1 / 30)
        except Exception:
            return

    return app


def start_background() -> bool:
    """Listen on localhost. Returns False when the token file is missing."""
    global _server
    if not load_token():
        logger.warning("Kuro API refused to listen: token file missing")
        return False
    if _server is not None:
        return True
    import uvicorn

    app = build_app()
    config = uvicorn.Config(
        app,
        host=_HOST,
        port=port(),
        log_level="warning",
        access_log=False,
    )
    server = uvicorn.Server(config)
    _server = server
    threading.Thread(target=server.run, name="KuroApi", daemon=True).start()
    logger.info("Kuro API listening on %s:%s", _HOST, port())
    return True
