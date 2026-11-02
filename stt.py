"""Speech-to-text for Kuro — CUDA (AC) / OpenVINO NPU (battery) / CPU.

KURO_STT_BACKEND=auto|cuda|npu|cpu (default auto).
In auto mode, power.get_power_status() is checked on every transcription:
  AC  -> faster-whisper small.en on CUDA
  DC  -> OpenVINO whisper-small.en INT8 on NPU (never NPU base for routing)
"""

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

import compute_policy
import config

logger = logging.getLogger(__name__)

_stt_lock = threading.Lock()
_cuda_model = None
_npu_pipe = None
_cpu_model = None
_active_backend: str | None = None  # last resolved backend name

_ROOT = Path(__file__).resolve().parent
_OV_SMALL = _ROOT / "bench" / "models" / "whisper-small.en-int8"

_INITIAL_PROMPT = (
    "Wake up daddy's home. Good night sir. Pause the song. Resume the song. "
    "Stop the music. What's the news today. Turn on the lights. Turn off the lights. "
    "Switch the lights off. Monitor off. Monitor on. Desk off. Desk on. "
    "Power on. Power off. Any urgent mail. Inbox check. Deadlines today. Hello Kuro."
)


def _site_packages() -> Path:
    return Path(sys.prefix) / "Lib" / "site-packages"


def configure_cuda_dlls() -> list[str]:
    """Make cublas64_12.dll / cudnn visible to CTranslate2 on Windows."""
    added: list[str] = []
    sp = _site_packages()
    candidates = [
        sp / "nvidia" / "cublas" / "bin",
        sp / "nvidia" / "cudnn" / "bin",
        sp / "nvidia" / "cuda_runtime" / "bin",
        sp / "nvidia" / "cuda_nvrtc" / "bin",
        sp / "torch" / "lib",
    ]
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


def resolve_backend() -> str:
    """Pick backend for this transcription via compute_policy (live power in auto)."""
    return compute_policy.prefer_stt()


def _build_cuda_model():
    from faster_whisper import WhisperModel

    import torch

    if not torch.cuda.is_available():
        raise RuntimeError("torch.cuda.is_available() is False")

    configure_cuda_dlls()
    if not _cublas12_loadable():
        raise RuntimeError("cublas64_12.dll not loadable after PATH fix")

    name = torch.cuda.get_device_name(0)
    # Policy: AC uses small.en. Allow override via env but prefer small.en.
    requested = (os.getenv("KURO_WHISPER_MODEL", "small.en").strip() or "small.en")
    compute = os.getenv("KURO_WHISPER_COMPUTE", "float16").strip() or "float16"
    candidates: list[str] = []
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

    logger.info("Loading Whisper small.en on CPU...")
    return WhisperModel("small.en", device="cpu", compute_type="int8")


def _ov_model_dir() -> Path:
    override = os.getenv("KURO_OV_WHISPER_DIR", "").strip()
    if override:
        return Path(override)
    return _OV_SMALL


def _build_npu_pipe():
    import openvino_genai as ov_genai

    model_dir = _ov_model_dir()
    if not model_dir.is_dir():
        raise FileNotFoundError(
            f"OpenVINO Whisper model missing at {model_dir}. "
            "Export whisper-small.en-int8 under bench/models/."
        )
    cache = _ROOT / "bench" / "cache"
    cache.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault("OV_CACHE_DIR", str(cache))
    # Policy: small.en INT8 on NPU only — never base for command routing.
    logger.info("Loading OpenVINO Whisper from %s on NPU...", model_dir)
    pipe = ov_genai.WhisperPipeline(str(model_dir), "NPU")
    warm_wav = _ROOT / "bench" / "data" / "wavs" / "cmd_hello.wav"
    if warm_wav.is_file():
        audio, _sr = _load_and_prep(str(warm_wav))
    else:
        audio = (np.sin(np.linspace(0, 200 * np.pi, 16000)).astype(np.float32) * 0.05)
    try:
        cfg = pipe.get_generation_config()
        cfg.language = "en"
        cfg.task = "transcribe"
        cfg.num_beams = 1
        cfg.initial_prompt = _INITIAL_PROMPT
        pipe.generate(audio, cfg)
    except Exception as e:
        logger.warning("NPU warm-up with config failed (%s); trying bare generate", e)
        pipe.generate(audio)
    logger.info("NPU Whisper warm-up complete")
    return pipe


def _unload_cuda() -> None:
    global _cuda_model
    _cuda_model = None
    try:
        import gc

        import torch

        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
    except Exception:
        pass


def _unload_npu() -> None:
    global _npu_pipe
    _npu_pipe = None
    try:
        import gc

        gc.collect()
    except Exception:
        pass


def _unload_cpu() -> None:
    global _cpu_model
    _cpu_model = None


def _ensure_backend(backend: str) -> str:
    """Load requested backend; fall back cuda→cpu, npu→cpu."""
    global _cuda_model, _npu_pipe, _cpu_model, _active_backend

    if backend == "cuda":
        if _cuda_model is None:
            try:
                _cuda_model = _build_cuda_model()
            except Exception as e:
                logger.error("CUDA STT failed (%s) — trying CPU", e)
                backend = "cpu"
        if backend == "cuda":
            # Free NPU when on AC so VRAM/NPU aren't both held needlessly
            if _npu_pipe is not None:
                _unload_npu()
            _active_backend = "cuda"
            return "cuda"

    if backend == "npu":
        if _npu_pipe is None:
            try:
                _npu_pipe = _build_npu_pipe()
            except Exception as e:
                logger.error("NPU STT failed (%s) — trying CPU", e)
                backend = "cpu"
        if backend == "npu":
            if _cuda_model is not None:
                _unload_cuda()
            _active_backend = "npu"
            return "npu"

    # cpu
    if _cpu_model is None:
        _cpu_model = _build_cpu_model()
    _active_backend = "cpu"
    return "cpu"


def force_cuda() -> bool:
    """Reload model on CUDA (diagnostics / recovery)."""
    with _stt_lock:
        _unload_cuda()
        try:
            _ensure_backend("cuda")
            return _active_backend == "cuda"
        except Exception:
            logger.exception("force_cuda failed")
            _ensure_backend("cpu")
            return False


def backend_label() -> str:
    return _active_backend or "unloaded"


# Compatibility for diagnostics that import _device_label
_device_label = "unloaded"


def _load_and_prep(audio_path: str) -> tuple[np.ndarray, int]:
    audio, sr = sf.read(audio_path, always_2d=False)
    audio = np.asarray(audio, dtype=np.float32)
    if audio.ndim > 1:
        audio = audio.mean(axis=1)

    if len(audio) > 8:
        rc = 1.0 / (2.0 * math.pi * 80.0)
        dt = 1.0 / float(sr)
        alpha = rc / (rc + dt)
        y = np.empty_like(audio)
        y[0] = audio[0]
        for i in range(1, len(audio)):
            y[i] = alpha * (y[i - 1] + audio[i] - audio[i - 1])
        audio = y

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


def _transcribe_faster_whisper(model, audio_input: str, *, partial: bool) -> str:
    vad_params = dict(
        threshold=0.3,
        min_speech_duration_ms=60,
        min_silence_duration_ms=250,
        speech_pad_ms=450,
    )
    beam = 1 if partial else int(os.getenv("KURO_WHISPER_BEAM", "5") or "5")
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
    return " ".join(" ".join(parts).split())


def _transcribe_ov(pipe, audio: np.ndarray, *, partial: bool) -> str:
    cfg = pipe.get_generation_config()
    cfg.language = "en"
    cfg.task = "transcribe"
    beam = 1 if partial else int(os.getenv("KURO_WHISPER_BEAM", "5") or "5")
    cfg.num_beams = max(1, beam)
    cfg.initial_prompt = _INITIAL_PROMPT
    try:
        result = pipe.generate(audio, cfg)
    except Exception:
        result = pipe.generate(audio)
    if hasattr(result, "texts") and result.texts:
        text = str(result.texts[0]).strip()
    elif hasattr(result, "text"):
        text = str(result.text).strip()
    else:
        text = str(result).strip()
    return " ".join(text.split())


def transcribe(audio_path: str, *, partial: bool = False) -> str:
    """Transcribe with the backend selected for *this* call (live power in auto)."""
    global _device_label
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
        prep_audio = audio
    except Exception as e:
        logger.warning("Audio prep failed (%s)", e)
        audio_input = audio_path
        prep_audio = None

    wanted = resolve_backend()
    with _stt_lock:
        backend = _ensure_backend(wanted)
        _device_label = backend
        try:
            if backend == "npu":
                if prep_audio is None:
                    prep_audio, _ = _load_and_prep(audio_input)
                text = _transcribe_ov(_npu_pipe, prep_audio, partial=partial)
            elif backend == "cuda":
                text = _transcribe_faster_whisper(_cuda_model, audio_input, partial=partial)
            else:
                text = _transcribe_faster_whisper(_cpu_model, audio_input, partial=partial)
        except Exception as e:
            logger.error("STT decode failed on %s: %s", backend, e)
            if backend != "cpu":
                try:
                    _ensure_backend("cpu")
                    _device_label = "cpu"
                    text = _transcribe_faster_whisper(_cpu_model, audio_input, partial=partial)
                except Exception:
                    logger.exception("CPU STT also failed")
                    text = ""
            else:
                text = ""

    if tmp:
        try:
            os.remove(tmp)
        except Exception:
            pass

    return text


# Eager CUDA DLL setup only — do not force-load a backend at import (auto needs live power).
logger.info("STT module ready (backend=%s)", getattr(config, "KURO_STT_BACKEND", "auto"))
try:
    configure_cuda_dlls()
except Exception:
    logger.exception("CUDA DLL configure failed at import")
