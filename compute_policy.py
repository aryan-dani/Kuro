"""Kuro compute policy — AC vs battery routing for STT/TTS/CUDA.

Modes:
  ac_full      — CUDA Whisper, XTTS allowed, dGPU ok
  battery_thin — OpenVINO NPU Whisper, Piper TTS, no CUDA keep-alive

Env overrides:
  KURO_STT_BACKEND=auto|cuda|npu|cpu
  KURO_TTS_ENGINE=auto|xtts|piper|pyttsx3
"""

from __future__ import annotations

import logging
import os
from typing import Literal

import config
import power

logger = logging.getLogger(__name__)

Mode = Literal["ac_full", "battery_thin"]
SttBackend = Literal["cuda", "npu", "cpu"]
TtsEngine = Literal["xtts", "piper", "pyttsx3"]


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


def should_preload_xtts() -> bool:
    """Warm XTTS at boot only on AC when engine wants natural voice."""
    raw = (getattr(config, "KURO_TTS_ENGINE", None) or os.getenv("KURO_TTS_ENGINE", "auto")).strip().lower()
    if raw == "xtts":
        return True
    if raw in ("piper", "pyttsx3"):
        return False
    return on_ac()


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
