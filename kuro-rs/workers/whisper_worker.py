"""Persistent faster-whisper worker. One JSON line in, one JSON line out.

Stdout is reserved for protocol lines. Logs go to stderr.
"""

from __future__ import annotations

import base64
import json
import os
import sys

import numpy as np


def _model_path() -> str:
    override = os.environ.get("KURO_CT2_MODEL", "").strip()
    if override:
        return override
    root = os.environ.get("USERPROFILE") or os.path.expanduser("~")
    snap = os.path.join(
        root,
        ".cache",
        "huggingface",
        "hub",
        "models--Systran--faster-whisper-small.en",
        "snapshots",
    )
    if os.path.isdir(snap):
        for name in os.listdir(snap):
            candidate = os.path.join(snap, name)
            if os.path.isfile(os.path.join(candidate, "model.bin")):
                return candidate
    return "small.en"


def main() -> None:
    try:
        from faster_whisper import WhisperModel

        device = os.environ.get("KURO_WHISPER_DEVICE", "cpu").strip() or "cpu"
        compute = "float16" if device == "cuda" else "int8"
        model = WhisperModel(_model_path(), device=device, compute_type=compute)
    except Exception as exc:
        sys.stdout.write(json.dumps({"ready": False, "error": str(exc)}) + "\n")
        sys.stdout.flush()
        return

    sys.stdout.write(json.dumps({"ready": True}) + "\n")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            raw = base64.b64decode(req.get("pcm") or "")
            pcm = np.frombuffer(raw, dtype=np.float32)
            rate = int(req.get("sample_rate") or 16000)
            if rate != 16000 and pcm.size:
                duration = pcm.size / float(rate)
                count = max(1, int(duration * 16000))
                src = np.linspace(0.0, 1.0, pcm.size, endpoint=False)
                dst = np.linspace(0.0, 1.0, count, endpoint=False)
                pcm = np.interp(dst, src, pcm).astype(np.float32)
            beam = int(req.get("beam") or 5)
            prompt = (req.get("prompt") or "").strip() or None
            segments, _info = model.transcribe(
                pcm,
                beam_size=max(1, beam),
                language="en",
                initial_prompt=prompt,
                vad_filter=False,
            )
            text = " ".join(seg.text.strip() for seg in segments).strip()
            sys.stdout.write(json.dumps({"text": text}) + "\n")
        except Exception as exc:
            sys.stdout.write(json.dumps({"text": "", "error": str(exc)}) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
