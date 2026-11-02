#!/usr/bin/env python3
"""Phase 3 gap: Piper vs XTTS first-audio latency.

Measures time from synth start until first PCM chunk is available.
Writes bench/tts_latency_results.json.
"""

from __future__ import annotations

import json
import os
import statistics
import sys
import tempfile
import time
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

OUT = ROOT / "bench" / "tts_latency_results.json"
TEXT = "Desk on. Welcome home, sir."


def _piper_first_audio() -> dict:
    try:
        from piper import PiperVoice
    except ImportError as e:
        return {"ok": False, "error": f"piper not installed: {e}"}

    # Prefer a local onnx if present; else skip download in bench
    model = os.getenv("KURO_PIPER_MODEL", "").strip()
    if not model:
        candidates = list((ROOT / "voices").glob("**/*.onnx")) + list(
            (ROOT / "bench" / "models" / "piper").glob("**/*.onnx")
        )
        if not candidates:
            return {
                "ok": False,
                "error": "No Piper .onnx found. Set KURO_PIPER_MODEL or place under voices/",
            }
        model = str(candidates[0])

    voice = PiperVoice.load(model)
    times = []
    for _ in range(5):
        t0 = time.perf_counter()
        first = None
        # piper synthesize streams audio chunks
        for chunk in voice.synthesize(TEXT):
            first = time.perf_counter()
            break
        if first is None:
            return {"ok": False, "error": "Piper produced no audio"}
        times.append(first - t0)
    return {
        "ok": True,
        "engine": "piper",
        "model": model,
        "first_audio_p50_sec": statistics.median(times),
        "first_audio_mean_sec": statistics.mean(times),
        "samples": times,
    }


def _xtts_first_audio() -> dict:
    # Use production tts path but capture synthesis wall time to first writable buffer.
    # XTTS in this repo synthesizes full utterance then plays — first-audio ~= full synth.
    try:
        import torch
        from TTS.api import TTS
    except ImportError as e:
        return {"ok": False, "error": f"XTTS deps missing: {e}"}

    if not torch.cuda.is_available():
        return {"ok": False, "error": "CUDA not available for XTTS"}

    device = "cuda"
    t_load = time.perf_counter()
    tts = TTS("tts_models/multilingual/multi-dataset/xtts_v2").to(device)
    load_sec = time.perf_counter() - t_load

    speaker = None
    ref = ROOT / "voices" / "kuro_voice.wav"
    times = []
    for i in range(3):
        fd, path = tempfile.mkstemp(suffix=".wav")
        os.close(fd)
        t0 = time.perf_counter()
        kwargs = {"text": TEXT, "file_path": path, "language": "en"}
        if ref.is_file():
            kwargs["speaker_wav"] = str(ref)
        else:
            # built-in speaker name if API supports it
            try:
                tts.tts_to_file(text=TEXT, file_path=path, language="en", speaker="Daisy Studious")
            except Exception:
                tts.tts_to_file(**kwargs)
            times.append(time.perf_counter() - t0)
            try:
                os.remove(path)
            except Exception:
                pass
            continue
        tts.tts_to_file(**kwargs)
        # first audio available when file exists with frames
        t1 = time.perf_counter()
        try:
            with wave.open(path, "rb") as w:
                _ = w.getnframes()
        except Exception:
            pass
        times.append(t1 - t0)
        try:
            os.remove(path)
        except Exception:
            pass

    return {
        "ok": True,
        "engine": "xtts_v2",
        "load_sec": load_sec,
        "first_audio_p50_sec": statistics.median(times) if times else None,
        "first_audio_mean_sec": statistics.mean(times) if times else None,
        "samples": times,
        "note": "XTTS path writes full WAV then returns; first-audio ~= full synthesis latency",
    }


def main() -> int:
    piper = _piper_first_audio()
    print("Piper:", piper)
    xtts = _xtts_first_audio()
    print("XTTS:", {k: xtts[k] for k in xtts if k != "samples"})
    payload = {
        "generated": time.strftime("%Y-%m-%d %H:%M:%S"),
        "text": TEXT,
        "piper": piper,
        "xtts": xtts,
    }
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print("Wrote", OUT)
    return 0 if (piper.get("ok") or xtts.get("ok")) else 1


if __name__ == "__main__":
    import time

    raise SystemExit(main())
