"""Ensure only one Kuro process owns the hotkey / TTS at a time."""

from __future__ import annotations

import atexit
import os
import sys
from pathlib import Path

_mutex_handle = None
_lock_path: Path | None = None


def acquire() -> bool:
    """
    Return True if this process is the sole Kuro instance.
    Must be called BEFORE heavy imports (torch/STT) so a second launch exits fast.
    """
    global _mutex_handle, _lock_path

    lock_dir = Path(os.environ.get("LOCALAPPDATA", ".")) / "Kuro"
    try:
        lock_dir.mkdir(parents=True, exist_ok=True)
    except Exception:
        pass
    _lock_path = lock_dir / "kuro.pid"

    try:
        import ctypes

        kernel32 = ctypes.windll.kernel32
        ERROR_ALREADY_EXISTS = 183
        name = "Global\\KuroAssistantSingleton.v1"
        handle = kernel32.CreateMutexW(None, True, name)  # take ownership immediately
        err = int(kernel32.GetLastError())
        if not handle:
            print("[Kuro] CreateMutex failed — refusing to start second copy", file=sys.stderr)
            return False
        if err == ERROR_ALREADY_EXISTS:
            kernel32.CloseHandle(handle)
            return False
        _mutex_handle = handle
        atexit.register(_release)
    except Exception as e:
        print(f"[Kuro] Singleton mutex error ({e}) — refusing start", file=sys.stderr)
        return False

    try:
        _lock_path.write_text(str(os.getpid()), encoding="utf-8")
    except Exception:
        pass
    return True


def _release() -> None:
    global _mutex_handle
    if _lock_path is not None:
        try:
            if _lock_path.exists():
                stored = _lock_path.read_text(encoding="utf-8").strip()
                if stored == str(os.getpid()):
                    _lock_path.unlink(missing_ok=True)
        except Exception:
            pass
    if _mutex_handle is not None:
        try:
            import ctypes

            ctypes.windll.kernel32.ReleaseMutex(_mutex_handle)
            ctypes.windll.kernel32.CloseHandle(_mutex_handle)
        except Exception:
            pass
        _mutex_handle = None


def refuse_and_exit() -> None:
    msg = (
        "Kuro is already running. "
        "Only one instance can own Ctrl+Alt+Space / TTS."
    )
    print(f"[Kuro] {msg}", file=sys.stderr)
    sys.exit(0)
