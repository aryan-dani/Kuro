"""Send WhatsApp through a linked device. No desktop window.

The session lives in %LOCALAPPDATA%\\Kuro\\wa_session.db. The first connect
opens a scan window. After the phone links it, later sends stay invisible.
"""

from __future__ import annotations

import logging
import os
import threading
import time

logger = logging.getLogger(__name__)

_LINK_SPOKEN = (
    "I opened a link code. On your phone, open WhatsApp, then Linked devices, "
    "then Link a device, and scan it. Then ask me again."
)

_lock = threading.Lock()
_send_lock = threading.Lock()
_ready = threading.Event()
_qr_open = threading.Event()
_thread: threading.Thread | None = None
_client = None


def session_path() -> str:
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return os.path.join(root, "Kuro", "wa_session.db")


def _session_likely() -> bool:
    try:
        return os.path.getsize(session_path()) > 4096
    except OSError:
        return False


def _show_link(path: str | None) -> None:
    """Draw the code on the HUD window. A second Tk loop makes the HUD flicker."""
    try:
        from overlay import current

        hud = current()
    except Exception:
        hud = None
    if hud is not None:
        hud.show_link_code(path)
        return
    if path:
        try:
            os.startfile(path)  # noqa: S606
        except Exception:
            logger.error("WhatsApp link picture could not be opened")


def _on_qr(_client_obj, data) -> None:
    try:
        import segno

        folder = os.path.dirname(session_path())
        os.makedirs(folder, exist_ok=True)
        path = os.path.join(folder, "wa_link.png")
        code = segno.make_qr(data)
        code.save(path, scale=8)
        _qr_open.set()
        _show_link(path)
        logger.info("WhatsApp link code shown")
    except Exception:
        logger.error("WhatsApp link code could not be drawn")


def _on_connected(_client_obj, _message) -> None:
    _ready.set()
    _qr_open.clear()
    _show_link(None)
    logger.info("WhatsApp linked device connected")


def _on_down(_client_obj, _message) -> None:
    _ready.clear()
    logger.info("WhatsApp linked device disconnected")


def _props():
    from neonize.proto.waCompanionReg.WAWebProtobufsCompanionReg_pb2 import DeviceProps

    history = DeviceProps.HistorySyncConfig(
        fullSyncDaysLimit=0,
        fullSyncSizeMbLimit=1,
        recentSyncDaysLimit=0,
        storageQuotaMb=16,
    )
    return DeviceProps(
        os="Kuro",
        platformType=DeviceProps.DESKTOP,
        requireFullSync=False,
        historySyncConfig=history,
    )


def _connect() -> None:
    global _client
    try:
        from neonize.client import NewClient

        os.makedirs(os.path.dirname(session_path()), exist_ok=True)
        client = NewClient(session_path(), props=_props(), uuid="kuro")
        client.event.qr(_on_qr)
        client.event.list_func[3] = _on_connected
        client.event.list_func[6] = _on_down
        client.event.list_func[12] = _on_down
        _client = client
        client.connect()
    except Exception:
        logger.error("WhatsApp link failed: %s", "connect")
        _ready.clear()


def warmup() -> None:
    """Start the linked device if it is not already running. Never raises."""
    global _thread
    try:
        with _lock:
            if _thread is not None and _thread.is_alive():
                return
            _thread = threading.Thread(target=_connect, name="kuro-wa", daemon=True)
            _thread.start()
    except Exception:
        logger.error("WhatsApp warmup failed")


def deliver(digits: str, *, text: str = "", image: bytes | None = None) -> str:
    """Send text or an image. Returns 'ok' or a speakable error. Never raises."""
    try:
        warmup()
        started = time.time()
        limit = 12.0 if _session_likely() else 6.0
        while time.time() < started + limit:
            if _ready.is_set() and _client is not None:
                break
            if _qr_open.is_set() and time.time() > started + 1.2:
                return _LINK_SPOKEN
            time.sleep(0.05)
        else:
            if _qr_open.is_set():
                return _LINK_SPOKEN
            return "WhatsApp is still linking. Try again in a moment."
        with _send_lock:
            if image:
                _client.send_image(digits, image)
            else:
                _client.send_message(digits, text)
        return "ok"
    except Exception:
        logger.error("WhatsApp send failed")
        return "I couldn't send that WhatsApp."
