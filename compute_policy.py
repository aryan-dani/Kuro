"""Kuro compute policy — AC vs battery routing for STT/TTS/CUDA.

Modes:
  ac_full      — CUDA Whisper, XTTS allowed, dGPU ok
  battery_thin — OpenVINO NPU Whisper, Piper TTS, no CUDA keep-alive

Env overrides:
  KURO_STT_BACKEND=auto|cuda|npu|cpu
  KURO_TTS_ENGINE=auto|xtts|piper|pyttsx3
"""

from __future__ import annotations

import json
import logging
import os
import time
from typing import Literal

import config
import power

logger = logging.getLogger(__name__)

Mode = Literal["ac_full", "battery_thin"]
SttBackend = Literal["cuda", "npu", "cpu"]
TtsEngine = Literal["xtts", "piper", "pyttsx3"]

# Lenovo GetSmartFanMode on this Legion: 1 quiet, 2 balanced, 3 performance.
_PERFORMANCE_MODE = 3
_LEGION_FILE = os.path.join(os.environ.get("LOCALAPPDATA", "."), "Kuro", "legion_mode.json")
_LEGION_MAX_AGE_SEC = 45.0


def get_power() -> dict:
    return power.get_power_status()


def on_ac() -> bool:
    st = get_power()
    return bool(st.get("is_charging") or st.get("ac_line_status") == 1)


def mode() -> Mode:
    return "ac_full" if on_ac() else "battery_thin"


def prefer_stt() -> SttBackend:
    """Backend for *this* transcription. Honors KURO_STT_BACKEND; auto uses live power."""
    raw = (getattr(config, "KURO_STT_BACKEND", None) or os.getenv("KURO_STT_BACKEND", "auto")).strip().lower()
    if raw in ("cuda", "npu", "cpu"):
        return raw  # type: ignore[return-value]
    return "cuda" if on_ac() else "npu"


def prefer_tts(*, fast: bool = False) -> TtsEngine:
    """TTS engine for *this* utterance."""
    raw = (getattr(config, "KURO_TTS_ENGINE", None) or os.getenv("KURO_TTS_ENGINE", "auto")).strip().lower()
    if raw in ("xtts", "piper", "pyttsx3"):
        return raw  # type: ignore[return-value]
    # auto
    if fast:
        return "piper" if _piper_available() else "pyttsx3"
    if on_ac():
        return "xtts"
    return "piper" if _piper_available() else "pyttsx3"


def allow_cuda() -> bool:
    """True when CUDA work is welcome (AC, or forced STT/TTS cuda/xtts)."""
    if on_ac():
        return True
    stt = (getattr(config, "KURO_STT_BACKEND", None) or "auto").strip().lower()
    tts = (getattr(config, "KURO_TTS_ENGINE", None) or os.getenv("KURO_TTS_ENGINE", "auto")).strip().lower()
    return stt == "cuda" or tts == "xtts"


def legion_status() -> dict | None:
    """Elevated writer drops this file. Missing or stale means stay light."""
    try:
        with open(_LEGION_FILE, encoding="utf-8") as f:
            data = json.load(f)
    except Exception:
        return None
    if not isinstance(data, dict):
        return None
    updated = data.get("updated")
    try:
        age = time.time() - float(updated)
    except (TypeError, ValueError):
        return None
    if age < 0 or age > _LEGION_MAX_AGE_SEC:
        return None
    return data


def ready_for_heavy() -> bool:
    """CUDA Whisper + XTTS preload only on the real charger in Performance mode.

    Live AC must be online. smart_fan_mode 3 is Performance.
    ac_fit 0 means Lenovo says the adapter is too weak. -1 means the read failed.
    """
    if not on_ac():
        return False
    status = legion_status()
    if not status:
        return False
    if not status.get("ac_online"):
        return False
    try:
        fan = int(status.get("smart_fan_mode"))
    except (TypeError, ValueError):
        return False
    if fan != _PERFORMANCE_MODE:
        return False
    try:
        fit = int(status.get("ac_fit"))
    except (TypeError, ValueError):
        fit = -1
    if fit == 0:
        return False
    return True


def should_preload_xtts() -> bool:
    """Background XTTS load only when the heavy gate is open and voice is wanted."""
    raw = (getattr(config, "KURO_TTS_ENGINE", None) or os.getenv("KURO_TTS_ENGINE", "auto")).strip().lower()
    if raw in ("piper", "pyttsx3"):
        return False
    return ready_for_heavy()


def _piper_available() -> bool:
    model = piper_model_path()
    if not model:
        return False
    try:
        import piper  # noqa: F401

        return True
    except ImportError:
        return False


def piper_model_path() -> str | None:
    override = os.getenv("KURO_PIPER_MODEL", "").strip()
    if override and os.path.isfile(override):
        return os.path.abspath(override)
    root = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(root, "bench", "models", "piper", "en_US-lessac-medium.onnx"),
        os.path.join(root, "voices", "en_US-lessac-medium.onnx"),
    ]
    for p in candidates:
        if os.path.isfile(p):
            return p
    return None


def device_report() -> dict:
    """Snapshot for boot diagnostics (replaces gpu_boost.device_report)."""
    info: dict = {
        "power": get_power(),
        "mode": mode(),
        "stt": prefer_stt(),
        "tts": prefer_tts(),
        "allow_cuda": allow_cuda(),
        "ready_for_heavy": ready_for_heavy(),
        "legion": legion_status(),
        "preload_xtts": should_preload_xtts(),
        "cuda": False,
    }
    try:
        import torch

        info["cuda"] = bool(torch.cuda.is_available())
        if info["cuda"]:
            info["gpu"] = torch.cuda.get_device_name(0)
            info["vram_allocated_mb"] = round(torch.cuda.memory_allocated(0) / (1024 * 1024), 1)
    except Exception as e:
        info["cuda_error"] = str(e)
    try:
        from stt import backend_label

        info["stt_loaded"] = backend_label()
    except Exception:
        pass
    return info
