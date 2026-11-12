"""Per-command stage timings. Off unless KURO_TIMING=1.

The CSV stores a command id, a kind, a stage name, and milliseconds.
It never stores transcript text, contact names, or secrets.
"""

from __future__ import annotations

import csv
import os
import threading
import time
import uuid
from pathlib import Path

_ENABLED = os.getenv("KURO_TIMING", "").strip().lower() in ("1", "true", "yes", "on")
_LOCK = threading.Lock()
_PATH_OVERRIDE: Path | None = None
_ACTIVE = ""
_OPEN: dict[str, dict] = {}


def enabled() -> bool:
    return _ENABLED


def csv_path() -> Path:
    if _PATH_OVERRIDE is not None:
        return _PATH_OVERRIDE
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return Path(root) / "Kuro" / "timing.csv"


def set_path(path: Path | None) -> None:
    """Tests point the CSV at a temp file. None restores the default."""
    global _PATH_OVERRIDE
    _PATH_OVERRIDE = path


def tick() -> float:
    return time.perf_counter()


def since(started: float) -> float:
    return (time.perf_counter() - started) * 1000.0


def once(kind: str, stage: str, ms: float) -> None:
    """One duration that is not part of a held command (model load, plug status)."""
    if not _ENABLED:
        return
    _append(uuid.uuid4().hex[:12], kind, stage, ms)


def begin(kind: str) -> str:
    """Open a command. The first mark should be hotkey_down."""
    global _ACTIVE
    if not _ENABLED:
        return ""
    command_id = uuid.uuid4().hex[:12]
    now = time.perf_counter()
    with _LOCK:
        _OPEN[command_id] = {"kind": kind, "t0": now, "last": now}
        _ACTIVE = command_id
    return command_id


def active() -> str:
    return _ACTIVE if _ENABLED else ""


def mark(stage: str, command_id: str = "") -> None:
    """Milliseconds since the previous mark on this command."""
    if not _ENABLED:
        return
    command_id = command_id or _ACTIVE
    if not command_id:
        return
    now = time.perf_counter()
    with _LOCK:
        row = _OPEN.get(command_id)
        if not row:
            return
        delta = (now - row["last"]) * 1000.0
        row["last"] = now
        kind = row["kind"]
    _append(command_id, kind, stage, delta)


def finish(command_id: str = "") -> None:
    if not _ENABLED:
        return
    global _ACTIVE
    command_id = command_id or _ACTIVE
    with _LOCK:
        _OPEN.pop(command_id, None)
        if _ACTIVE == command_id:
            _ACTIVE = ""


def _append(command_id: str, kind: str, stage: str, ms: float) -> None:
    path = csv_path()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        new_file = not path.is_file()
        with path.open("a", newline="", encoding="utf-8") as handle:
            writer = csv.writer(handle)
            if new_file:
                writer.writerow(["command_id", "kind", "stage", "ms"])
            writer.writerow([command_id, kind, stage, f"{ms:.1f}"])
    except Exception:
        return
