#!/usr/bin/env python3
"""Phase 3 gap: small LLM on NPU — compile time, tokens/sec, intent accuracy.

Tries OpenVINO GenAI LLMPipeline with a tiny instruct model if exportable.
Writes bench/llm_npu_results.json. Exits non-zero if model unavailable.
"""

from __future__ import annotations

import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts" / "bench"))

OUT = ROOT / "bench" / "llm_npu_results.json"
MODEL_DIR = ROOT / "bench" / "models" / "qwen2.5-0.5b-instruct-int4"
HF_ID = "Qwen/Qwen2.5-0.5B-Instruct"

PROMPTS = [
    ("Classify intent as one of: power_on, power_off, news, inbox, none.\nUser: turn on the lights\nIntent:", "power_on"),
    ("Classify intent as one of: power_on, power_off, news, inbox, none.\nUser: monitor off\nIntent:", "power_off"),
    ("Classify intent as one of: power_on, power_off, news, inbox, none.\nUser: what's the news today\nIntent:", "news"),
    ("Classify intent as one of: power_on, power_off, news, inbox, none.\nUser: any urgent mail\nIntent:", "inbox"),
    ("Classify intent as one of: power_on, power_off, news, inbox, none.\nUser: tell me a joke\nIntent:", "none"),
]


def _export() -> bool:
    if (MODEL_DIR / "openvino_model.xml").exists() or list(MODEL_DIR.glob("*.xml")):
        return True
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    cmd = [
        sys.executable,
        "-m",
        "optimum.exporters.openvino",
        "--model",
        HF_ID,
        "--trust-remote-code",
        "--weight-format",
        "int4",
        str(MODEL_DIR),
    ]
    print("EXPORT:", " ".join(cmd))
    return subprocess.run(cmd, cwd=str(ROOT)).returncode == 0


def _extract_intent(text: str) -> str:
    low = text.lower()
    for label in ("power_on", "power_off", "news", "inbox", "none"):
        if label in low:
            return label
    return "none"


def main() -> int:
    os.environ.setdefault("OV_CACHE_DIR", str(ROOT / "bench" / "cache"))
    if not _export():
        payload = {
            "ok": False,
            "error": "Could not export small LLM for NPU",
            "model": HF_ID,
        }
        OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(payload)
        return 1

    import openvino_genai as ov_genai

    t0 = time.perf_counter()
    try:
        pipe = ov_genai.LLMPipeline(str(MODEL_DIR), "NPU")
    except Exception as e:
        payload = {"ok": False, "error": f"NPU compile/load failed: {e}", "model": str(MODEL_DIR)}
        OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(payload)
        return 1
    compile_sec = time.perf_counter() - t0

    # warm
    _ = pipe.generate("Say hi.", max_new_tokens=8)

    tok_rates = []
    correct = 0
    rows = []
    for prompt, expected in PROMPTS:
        t1 = time.perf_counter()
        out = pipe.generate(prompt, max_new_tokens=16)
        dt = time.perf_counter() - t1
        text = out if isinstance(out, str) else str(out)
        # rough token count
        ntok = max(1, len(text.split()))
        tps = ntok / dt if dt > 0 else 0.0
        tok_rates.append(tps)
        pred = _extract_intent(text)
        ok = pred == expected
        correct += int(ok)
        rows.append(
            {
                "expected": expected,
                "pred": pred,
                "ok": ok,
                "output": text[:200],
                "sec": dt,
                "tokens_per_sec_approx": tps,
            }
        )
        print(f"  {expected} -> {pred} ({tps:.1f} tok/s approx)")

    payload = {
        "ok": True,
        "model": str(MODEL_DIR),
        "hf": HF_ID,
        "device": "NPU",
        "compile_sec": compile_sec,
        "tokens_per_sec_mean": statistics.mean(tok_rates) if tok_rates else None,
        "intent_accuracy": correct / len(PROMPTS),
        "intent_n": len(PROMPTS),
        "intent_correct": correct,
        "rows": rows,
        "note": "tokens/sec approximated from whitespace tokens / wall time",
    }
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps({k: payload[k] for k in ("compile_sec", "tokens_per_sec_mean", "intent_accuracy")}, indent=2))
    print("Wrote", OUT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
