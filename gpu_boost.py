"""Keep the discrete GPU awake and on CUDA even on battery.

Laptop GPUs (RTX 5070) drop into deep P-states on DC power. Models can still
be loaded on CUDA while clocks crawl — that's why battery feels much slower.
This module:
  - raises process priority
  - holds a small CUDA residency buffer so the GPU stays mapped
  - periodically runs a tiny matmul to bounce clocks out of deep sleep
  - logs power + device so we can verify CUDA-on-battery
"""

from __future__ import annotations

import logging
import threading
import time

logger = logging.getLogger(__name__)

_started = False
_stop = threading.Event()
_thread: threading.Thread | None = None
_residency = None  # torch tensor kept on GPU
_last_kick = 0.0


def device_report() -> dict:
    """Snapshot of power + CUDA state for diagnostics."""
    import power

    info: dict = {"power": power.get_power_status(), "cuda": False}
    try:
        import torch

        info["cuda"] = bool(torch.cuda.is_available())
        if info["cuda"]:
            info["gpu"] = torch.cuda.get_device_name(0)
            info["vram_allocated_mb"] = round(torch.cuda.memory_allocated(0) / (1024 * 1024), 1)
            info["vram_reserved_mb"] = round(torch.cuda.memory_reserved(0) / (1024 * 1024), 1)
    except Exception as e:
        info["error"] = str(e)
    try:
        from stt import _device_label  # type: ignore

        info["stt_device"] = _device_label
    except Exception:
        pass
    return info


def _raise_priority() -> None:
    try:
        import ctypes

        kernel32 = ctypes.windll.kernel32
        handle = kernel32.GetCurrentProcess()
        # HIGH_PRIORITY_CLASS — snappier scheduling without REALTIME risks
        kernel32.SetPriorityClass(handle, 0x00000080)
        logger.info("Process priority set to HIGH")
    except Exception as e:
        logger.debug("priority bump skipped: %s", e)


def _prevent_sleep() -> None:
    """Tell Windows we are doing continuous work (helps on battery)."""
    try:
        import ctypes

        ES_CONTINUOUS = 0x80000000
        ES_SYSTEM_REQUIRED = 0x00000001
        # Do NOT use ES_DISPLAY_REQUIRED — we don't want to force the screen on
        ctypes.windll.kernel32.SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)
        logger.info("Execution state: system required (GPU/CPU stay eligible)")
    except Exception as e:
        logger.debug("execution state skipped: %s", e)


def _ensure_residency() -> bool:
    """Allocate a small persistent CUDA buffer so the dGPU stays attached."""
    global _residency
    try:
        import torch

        if not torch.cuda.is_available():
            return False
        if _residency is None:
            # ~16MB — enough to pin the CUDA context without eating VRAM
            _residency = torch.empty((4, 1024, 1024), device="cuda", dtype=torch.float16)
            torch.cuda.synchronize()
            logger.info(
                "CUDA residency pinned on %s (allocated ~%.0f MB)",
                torch.cuda.get_device_name(0),
                torch.cuda.memory_allocated(0) / (1024 * 1024),
            )
        return True
    except Exception as e:
        logger.warning("CUDA residency failed: %s", e)
        return False


def kick(reason: str = "manual") -> None:
    """Force a brief CUDA burst — call on PTT press / before STT."""
    global _last_kick
    try:
        import torch

        if not torch.cuda.is_available():
            return
        _ensure_residency()
        a = torch.randn((512, 512), device="cuda", dtype=torch.float16)
        b = torch.randn((512, 512), device="cuda", dtype=torch.float16)
        c = a @ b
        torch.cuda.synchronize()
        del a, b, c
        _last_kick = time.time()
        logger.debug("GPU kick (%s)", reason)
    except Exception as e:
        logger.debug("GPU kick failed: %s", e)


def _loop() -> None:
    # First residency ASAP
    _ensure_residency()
    kick("boot")
    report = device_report()
    logger.info(
        "GPU boost online — cuda=%s stt=%s charging=%s batt=%s",
        report.get("cuda"),
        report.get("stt_device"),
        report.get("power", {}).get("is_charging"),
        report.get("power", {}).get("battery_percent"),
    )

    while not _stop.wait(4.0):
        # Keep clocks from camping in deep P-states between utterances
        if time.time() - _last_kick > 3.5:
            kick("keepalive")


def start() -> None:
    """Start background GPU boost (idempotent)."""
    global _started, _thread
    if _started:
        return
    _started = True
    _stop.clear()
    _raise_priority()
    _prevent_sleep()
    # Prefer Tensor Core friendly settings when CUDA is present
    try:
        import torch

        if torch.cuda.is_available():
            torch.backends.cudnn.benchmark = True
            torch.set_float32_matmul_precision("high")
    except Exception:
        pass
    _thread = threading.Thread(target=_loop, name="KuroGpuBoost", daemon=True)
    _thread.start()


def stop() -> None:
    global _started, _residency
    _stop.set()
    _started = False
    try:
        import ctypes

        ctypes.windll.kernel32.SetThreadExecutionState(0x80000000)  # ES_CONTINUOUS only
    except Exception:
        pass
    _residency = None
