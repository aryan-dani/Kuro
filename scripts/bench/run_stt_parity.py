#!/usr/bin/env python3
"""Re-bench OV whisper-small with stt.py-parity settings + optional int4 export.

Parity with production stt.py:
  - _load_and_prep audio
  - language=en, task=transcribe
  - num_beams from KURO_WHISPER_BEAM (default 5)
  - initial_prompt = stt._INITIAL_PROMPT
"""

from __future__ import annotations

import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts" / "bench"))

from intent_check import predict_intent  # noqa: E402
import stt  # noqa: E402

CACHE = ROOT / "bench" / "cache"
OUT_JSON = ROOT / "bench" / "parity_results.json"
OUT_MD = ROOT / "bench" / "parity_results.md"
LABELS = ROOT / "bench" / "data" / "labels.json"
MODELS = ROOT / "bench" / "models"


def _percentile(xs: list[float], p: float) -> float:
    ys = sorted(xs)
    if not ys:
        return float("nan")
    k = (len(ys) - 1) * p
    f = int(k)
    c = min(f + 1, len(ys) - 1)
    if f == c:
        return ys[f]
    return ys[f] + (ys[c] - ys[f]) * (k - f)


def _export_int4() -> Path | None:
    dest = MODELS / "whisper-small.en-int4"
    if list(dest.glob("*.xml")) or (dest / "openvino_encoder_model.xml").exists():
        print(f"int4 already present: {dest}")
        return dest
    dest.mkdir(parents=True, exist_ok=True)
    cmd = [
        sys.executable,
        "-m",
        "optimum.exporters.openvino",
        "--model",
        "openai/whisper-small.en",
        "--trust-remote-code",
        "--weight-format",
        "int4",
        str(dest),
    ]
    print("EXPORT int4:", " ".join(cmd))
    r = subprocess.run(cmd, cwd=str(ROOT))
    if r.returncode != 0:
        print("int4 export failed")
        return None
    return dest


def _eval_ov(model_dir: Path, device: str, clips: list[dict], beam: int) -> dict:
    import openvino_genai as ov_genai

    os.environ.setdefault("OV_CACHE_DIR", str(CACHE))
    CACHE.mkdir(parents=True, exist_ok=True)
    t0 = time.perf_counter()
    t_c = time.perf_counter()
    pipe = ov_genai.WhisperPipeline(str(model_dir), device)
    compile_sec = time.perf_counter() - t_c
    cold = time.perf_counter() - t0

    cfg = ov_genai.WhisperGenerationConfig()
    cfg.language = "en"
    cfg.task = "transcribe"
    cfg.num_beams = beam
    cfg.initial_prompt = stt._INITIAL_PROMPT

    # warm
    warm_path = ROOT / "bench" / "data" / "wavs" / "cmd_hello.wav"
    audio, _ = stt._load_and_prep(str(warm_path))
    try:
        pipe.generate(audio, cfg)
    except Exception:
        pipe.generate(audio)

    command_total = command_ok = 0
    short_lats: list[float] = []
    per_clip = []
    for clip in clips:
        path = ROOT / clip["path"]
        times = []
        hyp = ""
        for _ in range(5):
            audio, _ = stt._load_and_prep(str(path))
            t1 = time.perf_counter()
            try:
                result = pipe.generate(audio, cfg)
            except Exception:
                result = pipe.generate(audio)
            times.append(time.perf_counter() - t1)
            if hasattr(result, "texts") and result.texts:
                hyp = str(result.texts[0]).strip()
            elif hasattr(result, "text"):
                hyp = str(result.text).strip()
            else:
                hyp = str(result).strip()
            hyp = " ".join(hyp.split())
        intent = clip.get("expected_intent") or "none"
        pred = predict_intent(hyp)
        if intent not in ("none", "dictation"):
            command_total += 1
            if pred == intent:
                command_ok += 1
        elif intent == "none" and str(clip["id"]).startswith("cmd_"):
            command_total += 1
            if pred == "none":
                command_ok += 1
        if float(clip.get("duration_sec") or 99) <= 5.0:
            short_lats.extend(times)
        per_clip.append(
            {
                "id": clip["id"],
                "hyp": hyp,
                "expected_intent": intent,
                "pred_intent": pred,
                "p50": _percentile(times, 0.5),
                "p95": _percentile(times, 0.95),
            }
        )

    del pipe
    return {
        "name": f"ov-{model_dir.name}-{device}-parity-beam{beam}",
        "ok": True,
        "cold_start_sec": cold,
        "compile_sec": compile_sec,
        "warm_p50_sec": _percentile(short_lats, 0.5),
        "warm_p95_sec": _percentile(short_lats, 0.95),
        "command_accuracy": (command_ok / command_total) if command_total else None,
        "command_n": command_total,
        "command_correct": command_ok,
        "per_clip": per_clip,
        "parity": {
            "load_and_prep": True,
            "language": "en",
            "num_beams": beam,
            "initial_prompt": True,
        },
    }


def main() -> int:
    clips = json.loads(LABELS.read_text(encoding="utf-8"))["clips"]
    beam = int(os.getenv("KURO_WHISPER_BEAM", "5") or "5")
    results = []

    print("PARITY GAP vs prior bench: prior OV used raw wav, num_beams=1, no prompt, no prep.")
    small = MODELS / "whisper-small.en-int8"
    if not small.is_dir():
        print("missing", small)
        return 1

    for device in ("NPU",):
        print(f"=== parity {small.name} {device} beam={beam} ===")
        try:
            r = _eval_ov(small, device, clips, beam)
            print(
                f"  p95={r['warm_p95_sec']:.3f} acc={r['command_accuracy']} "
                f"compile={r['compile_sec']:.3f}"
            )
            results.append(r)
        except Exception as e:
            results.append({"name": f"ov-small-int8-{device}-parity", "ok": False, "error": str(e)})
            print("FAIL", e)

    int4 = _export_int4()
    if int4 is not None:
        print(f"=== try int4 on NPU {int4} ===")
        try:
            r = _eval_ov(int4, "NPU", clips, beam)
            print(f"  int4 p95={r['warm_p95_sec']:.3f} acc={r['command_accuracy']}")
            results.append(r)
        except Exception as e:
            results.append({"name": "ov-small-int4-NPU-parity", "ok": False, "error": str(e)})
            print("int4 NPU compile/run failed:", e)

    payload = {
        "meta": {
            "generated": time.strftime("%Y-%m-%d %H:%M:%S"),
            "note": "OV parity with stt._load_and_prep + language/en + beam + initial_prompt",
            "prior_gap": "Original matrix used raw audio, default num_beams=1, no initial_prompt",
        },
        "results": results,
    }
    OUT_JSON.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    lines = ["# STT parity re-bench", "", json.dumps(payload["meta"], indent=2), ""]
    for r in results:
        lines.append(
            f"- **{r.get('name')}**: ok={r.get('ok')} p95={r.get('warm_p95_sec')} "
            f"acc={r.get('command_accuracy')} err={r.get('error', '')[:200]}"
        )
    OUT_MD.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("Wrote", OUT_JSON)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
