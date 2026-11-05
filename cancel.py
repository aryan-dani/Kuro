"""Shared cancel flag and a window that ignores synthetic keyboard events."""

from __future__ import annotations

import threading
import time

event = threading.Event()
synthetic_keys_until = 0.0


def arm_synthetic(seconds: float) -> None:
    """Ignore Esc and hotkey state changes until this many seconds from now."""
    global synthetic_keys_until
    synthetic_keys_until = max(synthetic_keys_until, time.time() + seconds)


def synthetic_active() -> bool:
    return time.time() < synthetic_keys_until
