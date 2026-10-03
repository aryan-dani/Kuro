"""Speech-to-text for Kuro — CUDA/cuBLAS accelerated on RTX GPUs."""

from __future__ import annotations

import logging
import math
import os
import sys
import tempfile
import threading
from pathlib import Path

import numpy as np
import soundfile as sf

logger = logging.getLogger(__name__)

_stt_lock = threading.Lock()
_model = None
_device_label = "cpu"

_INITIAL_PROMPT = (
    "Wake up daddy's home. Good night sir. Pause the song. Resume the song. "
    "Stop the music. What's the news today. Turn on the lights. Turn off the lights. "
    "Switch the lights off. Monitor off. Monitor on. Desk off. Desk on. "
    "Power on. Power off. Hello Kuro."
)


def _site_packages() -> Path:
    # Prefer venv site-packages even if cwd differs
    return Path(sys.prefix) / "Lib" / "site-packages"


def configure_cuda_dlls() -> list[str]:
    """
    Make cublas64_12.dll / cudnn visible to CTranslate2 on Windows.
    Torch may ship CUDA 13 libs; faster-whisper needs the cu12 wheels on PATH.
    """
    added: list[str] = []
    sp = _site_packages()
    candidates = [
        sp / "nvidia" / "cublas" / "bin",
        sp / "nvidia" / "cudnn" / "bin",
        sp / "nvidia" / "cuda_runtime" / "bin",
        sp / "nvidia" / "cuda_nvrtc" / "bin",
        sp / "torch" / "lib",
    ]
    # Any other nvidia/*/bin folders
    nvidia_root = sp / "nvidia"
    if nvidia_root.is_dir():
        for p in nvidia_root.glob("*/bin"):
            if p.is_dir() and p not in candidates:
                candidates.append(p)

    for raw in candidates:
        path = raw.resolve()
        if not path.is_dir():
            continue
        try:
            if hasattr(os, "add_dll_directory"):
                os.add_dll_directory(str(path))
            added.append(str(path))
        except OSError as e:
            logger.debug("add_dll_directory failed for %s: %s", path, e)

    if added:
        os.environ["PATH"] = os.pathsep.join(added) + os.pathsep + os.environ.get("PATH", "")
        logger.info("CUDA DLL paths configured (%d dirs)", len(added))
    return added


def _cublas12_loadable() -> bool:
    try:
        import ctypes

        ctypes.WinDLL("cublas64_12.dll")
        return True
    except OSError as e:
        logger.warning("cublas64_12.dll still not loadable: %s", e)
        return False


def _build_cuda_model():
    from faster_whisper import WhisperModel

    import torch

    if not torch.cuda.is_available():
        raise RuntimeError("torch.cuda.is_available() is False")

    configure_cuda_dlls()
    if not _cublas12_loadable():
        raise RuntimeError("cublas64_12.dll not loadable after PATH fix")

    name = torch.cuda.get_device_name(0)
    # Prefer medium.en when already cached; never block boot on a fresh HF download.
    requested = os.getenv("KURO_WHISPER_MODEL", "medium.en").strip() or "medium.en"
    compute = os.getenv("KURO_WHISPER_COMPUTE", "float16").strip() or "float16"
    candidates = []
    for mid in (requested, "small.en"):
        if mid not in candidates:
            candidates.append(mid)

    last_err: Exception | None = None
    model = None
    model_id = "small.en"
    for mid in candidates:
        try:
            local_only = mid != "small.en"
            logger.info(
                "Loading Whisper %s on CUDA (%s, %s, local_files_only=%s)...",
                mid,
                name,
                compute,
                local_only,
            )
            model = WhisperModel(
                mid,
                device="cuda",
                device_index=0,
                compute_type=compute,
                local_files_only=local_only,
            )
            model_id = mid
            break
        except Exception as e:
            last_err = e
            logger.warning("Whisper %s unavailable (%s)", mid, e)
            model = None

    if model is None:
        logger.info("Falling back to downloadable small.en")
        model = WhisperModel(
            "small.en",
            device="cuda",
            device_index=0,
            compute_type=compute,
            local_files_only=False,
        )
        model_id = "small.en"
        if last_err:
            logger.debug("Prior load errors: %s", last_err)

    # Warm-up so first real utterance isn't cold
    try:
        fd, tmp = tempfile.mkstemp(suffix=".wav")
        os.close(fd)
        tone = (np.sin(np.linspace(0, 200 * np.pi, 8000)).astype(np.float32) * 0.05)
        sf.write(tmp, tone, 16000)
        list(model.transcribe(tmp, beam_size=1, vad_filter=False))
        os.remove(tmp)
        logger.info("CUDA Whisper warm-up complete (%s)", model_id)
    except Exception as e:
        logger.warning("CUDA warm-up skipped: %s", e)
    return model


def _build_cpu_model():
    from faster_whisper import WhisperModel

    logger.info("Loading Whisper base.en on CPU (fallback)...")
    return WhisperModel("base.en", device="cpu", compute_type="int8")


def _get_model():
    global _model, _device_label
    if _model is not None:
        return _model
    try:
        _model = _build_cuda_model()
        _device_label = "cuda"
        logger.info("STT ready on CUDA — using GPU VRAM")
    except Exception as e:
        logger.error("CUDA STT failed (%s) — CPU fallback", e)
        _model = _build_cpu_model()
        _device_label = "cpu"
        logger.info("STT ready on CPU")
    return _model


def force_cuda() -> bool:
    """Reload model on CUDA (for diagnostics / recovery)."""
    global _model, _device_label
    with _stt_lock:
        _model = None
        try:
            _model = _build_cuda_model()
            _device_label = "cuda"
            return True
        except Exception:
            logger.exception("force_cuda failed")
            _model = _build_cpu_model()
            _device_label = "cpu"
            return False


logger.info("Initializing STT...")
try:
    # Configure DLLs before any ctranslate2 import side effects
    configure_cuda_dlls()
    _get_model()
except Exception:
    logger.exception("STT init failed")


def _load_and_prep(audio_path: str) -> tuple[np.ndarray, int]:
    audio, sr = sf.read(audio_path, always_2d=False)
    audio = np.asarray(audio, dtype=np.float32)
    if audio.ndim > 1:
        audio = audio.mean(axis=1)

    # Soft DC / rumble remove (simple 1-pole highpass ~80Hz)
    if len(audio) > 8:
        rc = 1.0 / (2.0 * math.pi * 80.0)
        dt = 1.0 / float(sr)
        alpha = rc / (rc + dt)
        y = np.empty_like(audio)
        y[0] = audio[0]
        for i in range(1, len(audio)):
            y[i] = alpha * (y[i - 1] + audio[i] - audio[i - 1])
        audio = y

    # Light noise gate on near-silence
    rms = float(np.sqrt(np.mean(audio * audio) + 1e-12))
    if rms > 0:
        gate = max(0.0, min(1.0, (rms - 0.004) / 0.012))
        audio = audio * (0.35 + 0.65 * gate)

    peak = float(np.max(np.abs(audio))) + 1e-8
    target = 0.55
    audio = np.clip(audio * min(target / peak, 12.0), -1.0, 1.0)

    pad = int(sr * 0.3)
    audio = np.pad(audio, (pad, pad), mode="constant")
    return audio, int(sr)


def transcribe(audio_path: str, *, partial: bool = False) -> str:
    """GPU-accelerated transcription. Returns '' on failure."""
    tmp = None
    try:
        audio, sr = _load_and_prep(audio_path)
        if float(np.max(np.abs(audio))) < 0.008:
            logger.info("STT skipped — clip too quiet")
            return ""
        fd, tmp = tempfile.mkstemp(suffix=".wav")
        os.close(fd)
        sf.write(tmp, audio, sr)
        audio_input = tmp
    except Exception as e:
        logger.warning("Audio prep failed (%s)", e)
        audio_input = audio_path

    vad_params = dict(
        threshold=0.3,
        min_speech_duration_ms=60,
        min_silence_duration_ms=250,
        speech_pad_ms=450,
    )

    # Final pass: wider beam for accuracy; partial stays greedy/fast
    beam = 1 if partial else int(os.getenv("KURO_WHISPER_BEAM", "5") or "5")

    with _stt_lock:
        model = _get_model()
        try:
            segments, _info = model.transcribe(
                audio_input,
                language="en",
                beam_size=beam,
                best_of=1 if partial else min(max(beam, 1), 5),
                temperature=0.0 if partial else [0.0, 0.2],
                vad_filter=not partial,
                vad_parameters=vad_params,
                condition_on_previous_text=False,
                initial_prompt=_INITIAL_PROMPT,
                word_timestamps=False,
                without_timestamps=True,
                compression_ratio_threshold=2.6,
                log_prob_threshold=-1.2,
                no_speech_threshold=0.45,
            )
            parts = [seg.text.strip() for seg in segments if seg.text and seg.text.strip()]
        except Exception as e:
            msg = str(e)
            logger.error("STT decode failed on %s: %s", _device_label, msg)
            if _device_label == "cuda":
                # One-shot recovery to CPU so Kuro keeps working, then try CUDA reload later
                global _model
                _model = _build_cpu_model()
                _device_label = "cpu"
                try:
                    segments, _info = _model.transcribe(
                        audio_input,
                        language="en",
                        beam_size=1,
                        best_of=1,
                        temperature=0.0,
                        vad_filter=True,
                        vad_parameters=vad_params,
                        condition_on_previous_text=False,
                        initial_prompt=_INITIAL_PROMPT,
                        without_timestamps=True,
                    )
                    parts = [seg.text.strip() for seg in segments if seg.text and seg.text.strip()]
                except Exception:
                    logger.exception("CPU STT also failed")
                    parts = []
            else:
                parts = []

    if tmp:
        try:
            os.remove(tmp)
        except Exception:
            pass

    return " ".join(" ".join(parts).split())
