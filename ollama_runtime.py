"""Start and wait for local Ollama only when the LLM path is actually needed."""

from __future__ import annotations

import logging
import os
import shutil
import subprocess
import time
import urllib.error
import urllib.request

logger = logging.getLogger(__name__)

_TAGS = "http://127.0.0.1:11434/api/tags"
_DEFAULT_WAIT = 45.0


def is_up(timeout: float = 1.2) -> bool:
    try:
        with urllib.request.urlopen(_TAGS, timeout=timeout) as resp:
            return 200 <= int(getattr(resp, "status", 200) or 200) < 300
    except Exception:
        return False


def _ollama_exe() -> str | None:
    local = os.path.join(
        os.environ.get("LOCALAPPDATA") or "",
        "Programs",
        "Ollama",
        "ollama.exe",
    )
    if local and os.path.isfile(local):
        return local
    found = shutil.which("ollama")
    return found


def start_serve() -> bool:
    """Launch `ollama serve` if needed. True if already up or launch was attempted."""
    if is_up():
        return True
    exe = _ollama_exe()
    if not exe:
        logger.warning("Ollama executable not found")
        return False
    try:
        creation = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        subprocess.Popen(
            [exe, "serve"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=creation,
        )
        logger.info("Started ollama serve via %s", exe)
        return True
    except Exception:
        logger.exception("Failed to start ollama serve")
        return False


def ensure_ready(timeout: float = _DEFAULT_WAIT) -> bool:
    """Ensure the HTTP API answers. Starts serve if down. Blocks until ready or timeout."""
    if is_up():
        return True
    if not start_serve():
        return False
    deadline = time.monotonic() + max(3.0, timeout)
    while time.monotonic() < deadline:
        if is_up(timeout=1.0):
            logger.info("Ollama API ready")
            return True
        time.sleep(0.4)
    logger.warning("Ollama did not become ready within %.1fs", timeout)
    return False
