#!/usr/bin/env python3
"""STT backend matrix for Kuro NPU thin-client gate.

Never calls phrases.try_handle. Writes bench/results.json and bench/results.md.
"""

from __future__ import annotations

import gc
import json
import os
import statistics
import sys
import time
import traceback
from dataclasses import asdict, dataclass, field
from pathlib import Path

import numpy as np
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts" / "bench"))

from intent_check import predict_intent  # noqa: E402

CACHE_DIR = ROOT / "bench" / "cache"
RESULTS_JSON = ROOT / "bench" / "results.json"
RESULTS_MD = ROOT / "bench" / "results.md"
LABELS = ROOT / "bench" / "data" / "labels.json"
OV_MODELS = ROOT / "bench" / "models"

os.environ.setdefault("OV_CACHE_DIR", str(CACHE_DIR))
CACHE_DIR.mkdir(parents=True, exist_ok=True)


@dataclass
class BackendResult:
    name: str
    ok: bool
    error: str = ""
    cold_start_sec: float | None = None
    compile_sec: float | None = None
    warm_p50_sec: float | None = None
    warm_p95_sec: float | None = None
    wer: float | None = None
    command_accuracy: float | None = None
    command_n: int = 0
    command_correct: int = 0
    discharge_idle_mw: float | None = None
    discharge_busy_mw: float | None = None
    power_note: str = ""
    per_clip: list[dict] = field(default_factory=list)
    stability_50: dict | None = None


def _load_labels() -> list[dict]:
    data = json.loads(LABELS.read_text(encoding="utf-8"))
    return data["clips"]


def _wer(ref: str, hyp: str) -> float:
    if not ref.strip():
        return 0.0 if not hyp.strip() else 1.0
    try:
        from jiwer import wer

        return float(wer(ref.lower(), hyp.lower()))
    except Exception:
        # Simple token error rate fallback
        r = ref.lower().split()
        h = hyp.lower().split()
        if not r:
            return 0.0
        # Levenshtein on tokens
        n, m = len(r), len(h)
        dp = [[0] * (m + 1) for _ in range(n + 1)]
        for i in range(n + 1):
            dp[i][0] = i
        for j in range(m + 1):
            dp[0][j] = j
        for i in range(1, n + 1):
            for j in range(1, m + 1):
                cost = 0 if r[i - 1] == h[j - 1] else 1
                dp[i][j] = min(dp[i - 1][j] + 1, dp[i][j - 1] + 1, dp[i - 1][j - 1] + cost)
        return dp[n][m] / n


def _read_wav(path: Path) -> tuple[np.ndarray, int]:
    audio, sr = sf.read(str(path), dtype="float32")
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    return audio.astype(np.float32), int(sr)


def _battery_discharge_rate_mw() -> tuple[float | None, str]:
    """Return DischargeRate (mW) or None with note."""
    try:
        import subprocess

        ps = (
            "Get-CimInstance -Namespace root/wmi -ClassName BatteryStatus "
            "| Select-Object -ExpandProperty DischargeRate"
        )
        out = subprocess.check_output(
            ["powershell", "-NoProfile", "-Command", ps],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
        if not out:
            return None, "BatteryStatus empty"
        val = float(out.splitlines()[-1].strip())
        return val, "WMI BatteryStatus.DischargeRate (mW)"
    except Exception as e:
        return None, f"telemetry unavailable: {e}"


def _sample_discharge(seconds: int = 60) -> tuple[float | None, str]:
    samples: list[float] = []
    note = ""
    t0 = time.time()
    while time.time() - t0 < seconds:
        rate, note = _battery_discharge_rate_mw()
        if rate is not None and rate > 0:
            samples.append(rate)
        time.sleep(2.0)
    if not samples:
        return None, note or "no positive DischargeRate samples (plugged in?)"
    return float(statistics.mean(samples)), note


def _percentile(xs: list[float], p: float) -> float:
    if not xs:
        return float("nan")
    ys = sorted(xs)
    k = (len(ys) - 1) * p
    f = int(k)
    c = min(f + 1, len(ys) - 1)
    if f == c:
        return ys[f]
    return ys[f] + (ys[c] - ys[f]) * (k - f)


# --- Backends -----------------------------------------------------------------


class FasterWhisperBackend:
    def __init__(self, model_id: str, device: str = "cuda", compute_type: str = "float16"):
        self.model_id = model_id
        self.device = device
        self.compute_type = compute_type
        self.model = None
        self.name = f"fw-{model_id}-{device}-{compute_type}"

    def load(self) -> float:
        t0 = time.perf_counter()
        from faster_whisper import WhisperModel

        import torch

        # Reuse Kuro CUDA DLL fix when on CUDA
        if self.device == "cuda":
            import stt as kuro_stt

            kuro_stt.configure_cuda_dlls()
            if not torch.cuda.is_available():
                raise RuntimeError("torch.cuda.is_available() is False")
        # Prefer local cache; medium.en is often incomplete on this machine.
        # Never block the bench on a multi-GB redownload.
        local_only = self.model_id != "small.en"
        try:
            self.model = WhisperModel(
                self.model_id,
                device=self.device,
                compute_type=self.compute_type,
                local_files_only=local_only,
            )
        except Exception as e:
            if self.model_id == "medium.en":
                raise RuntimeError(f"medium.en unavailable (incomplete cache): {e}") from e
            self.model = WhisperModel(
                self.model_id,
                device=self.device,
                compute_type=self.compute_type,
                local_files_only=False,
            )
        # warm
        tone = (np.sin(np.linspace(0, 200 * np.pi, 8000)).astype(np.float32) * 0.05)
        tmp = ROOT / "bench" / "cache" / "_warm.wav"
        sf.write(str(tmp), tone, 16000)
        list(self.model.transcribe(str(tmp), beam_size=1, vad_filter=False))
        return time.perf_counter() - t0

    def transcribe(self, path: Path) -> str:
        segments, _info = self.model.transcribe(str(path), beam_size=1, vad_filter=False)
        return " ".join(s.text.strip() for s in segments).strip()

    def unload(self) -> None:
        self.model = None
        gc.collect()
        try:
            import torch

            if torch.cuda.is_available():
                torch.cuda.empty_cache()
        except Exception:
            pass


class OpenVINOWhisperBackend:
    def __init__(self, model_dir: Path, device: str):
        self.model_dir = model_dir
        self.device = device
        self.pipe = None
        self.name = f"ov-{model_dir.name}-{device}"
        self.compile_sec: float | None = None

    def load(self) -> float:
        t0 = time.perf_counter()
        import openvino_genai as ov_genai

        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        t_compile = time.perf_counter()
        self.pipe = ov_genai.WhisperPipeline(str(self.model_dir), self.device)
        self.compile_sec = time.perf_counter() - t_compile
        # warm
        audio, sr = _read_wav(ROOT / "bench" / "data" / "wavs" / "cmd_hello.wav")
        _ = self.pipe.generate(audio.tolist() if hasattr(audio, "tolist") else audio)
        return time.perf_counter() - t0

    def transcribe(self, path: Path) -> str:
        audio, sr = _read_wav(path)
        try:
            result = self.pipe.generate(audio)
        except TypeError:
            result = self.pipe.generate(audio.tolist())
        if hasattr(result, "texts") and result.texts:
            return str(result.texts[0]).strip()
        if hasattr(result, "text"):
            return str(result.text).strip()
        if isinstance(result, str):
            return result.strip()
        s = str(result).strip()
        # WhisperDecodedResults str often looks like " Hello"
        return s

    def unload(self) -> None:
        self.pipe = None
        gc.collect()


def _find_ov_model(name: str) -> Path | None:
    p = OV_MODELS / name
    if not p.is_dir():
        return None
    xmls = list(p.glob("*.xml")) + list(p.glob("openvino_*.xml"))
    if xmls or (p / "openvino_encoder_model.xml").exists() or (p / "openvino_model.xml").exists():
        return p
    return None


def evaluate_backend(backend, clips: list[dict], repeats: int = 5) -> BackendResult:
    res = BackendResult(name=backend.name, ok=False)
    try:
        cold = backend.load()
        res.cold_start_sec = cold
        if getattr(backend, "compile_sec", None) is not None:
            res.compile_sec = backend.compile_sec
    except Exception as e:
        res.error = f"{type(e).__name__}: {e}\n{traceback.format_exc()}"
        backend.unload()
        return res

    # Idle discharge sample (short if already measured externally)
    # Per-clip warm latency
    command_total = 0
    command_ok = 0
    wers: list[float] = []
    all_latencies: list[float] = []

    short_clips = [c for c in clips if c.get("expected_intent") != "dictation"]
    for clip in clips:
        path = ROOT / clip["path"]
        expected = clip.get("expected_text") or ""
        intent = clip.get("expected_intent") or "none"
        times: list[float] = []
        hyp = ""
        for _ in range(repeats):
            t0 = time.perf_counter()
            hyp = backend.transcribe(path)
            times.append(time.perf_counter() - t0)
        all_latencies.extend(times)
        w = _wer(expected, hyp) if expected else None
        if w is not None:
            wers.append(w)
        pred = predict_intent(hyp)
        # Only score command accuracy for labeled intents that are not free/dictation noise
        if intent not in ("none", "dictation"):
            command_total += 1
            if pred == intent:
                command_ok += 1
        elif intent == "none" and clip["id"].startswith("cmd_"):
            command_total += 1
            if pred == "none":
                command_ok += 1
        res.per_clip.append(
            {
                "id": clip["id"],
                "hyp": hyp,
                "expected_text": expected,
                "expected_intent": intent,
                "pred_intent": pred,
                "p50": _percentile(times, 0.5),
                "p95": _percentile(times, 0.95),
                "wer": w,
            }
        )

    # Warm stats on clips <= 5s for gate
    short_lats: list[float] = []
    for clip, row in zip(clips, res.per_clip):
        if float(clip.get("duration_sec") or 99) <= 5.0:
            # approximate: use p50 of that clip as one sample; also keep all_latencies filtered
            short_lats.append(row["p50"])

    # Better: re-collect from repeats only for short clips
    short_all: list[float] = []
    for clip in clips:
        if float(clip.get("duration_sec") or 99) > 5.0:
            continue
        path = ROOT / clip["path"]
        for _ in range(3):
            t0 = time.perf_counter()
            backend.transcribe(path)
            short_all.append(time.perf_counter() - t0)

    lat_src = short_all or all_latencies
    res.warm_p50_sec = _percentile(lat_src, 0.5)
    res.warm_p95_sec = _percentile(lat_src, 0.95)
    res.wer = float(statistics.mean(wers)) if wers else None
    res.command_n = command_total
    res.command_correct = command_ok
    res.command_accuracy = (command_ok / command_total) if command_total else None
    res.ok = True
    backend.unload()
    return res


def stability_50(backend, clips: list[dict]) -> dict:
    short = [c for c in clips if float(c.get("duration_sec") or 99) <= 5.0][:10]
    if not short:
        return {"ok": False, "error": "no short clips"}
    try:
        backend.load()
    except Exception as e:
        return {"ok": False, "error": str(e)}
    errors = []
    for i in range(50):
        clip = short[i % len(short)]
        try:
            backend.transcribe(ROOT / clip["path"])
        except Exception as e:
            errors.append({"i": i, "error": f"{type(e).__name__}: {e}"})
            break
    backend.unload()
    return {"ok": len(errors) == 0, "runs": 50 if not errors else errors[0]["i"], "errors": errors}


def write_md(results: list[BackendResult], meta: dict) -> None:
    lines = [
        "# Kuro STT bench results",
        "",
        f"Generated: {meta.get('generated')}",
        f"Power: {meta.get('power')}",
        "",
        "| Backend | OK | Cold s | Compile s | Warm p50 | Warm p95 | WER | Cmd acc | Idle mW | Busy mW |",
        "|---------|----|--------|-----------|----------|----------|-----|---------|---------|---------|",
    ]
    for r in results:
        lines.append(
            "| {name} | {ok} | {cold} | {comp} | {p50} | {p95} | {wer} | {acc} | {idle} | {busy} |".format(
                name=r.name,
                ok=r.ok,
                cold=_fmt(r.cold_start_sec),
                comp=_fmt(r.compile_sec),
                p50=_fmt(r.warm_p50_sec),
                p95=_fmt(r.warm_p95_sec),
                wer=_fmt(r.wer),
                acc=_fmt(r.command_accuracy),
                idle=_fmt(r.discharge_idle_mw),
                busy=_fmt(r.discharge_busy_mw),
            )
        )
    lines.append("")
    lines.append("## Errors")
    for r in results:
        if r.error:
            lines.append(f"### {r.name}")
            lines.append("```")
            lines.append(r.error[:4000])
            lines.append("```")
    if any(r.stability_50 for r in results):
        lines.append("")
        lines.append("## NPU stability (50 runs)")
        for r in results:
            if r.stability_50:
                lines.append(f"- {r.name}: `{json.dumps(r.stability_50)}`")
    lines.append("")
    lines.append("## Gate rule")
    lines.append(
        "NPU wins on battery if warm p95 (clips <=5s) < 1.5s, "
        "command accuracy within 3 points of CUDA baseline, and 50-run check did not crash."
    )
    RESULTS_MD.write_text("\n".join(lines), encoding="utf-8")


def _fmt(x) -> str:
    if x is None:
        return "-"
    if isinstance(x, bool):
        return str(x)
    try:
        return f"{float(x):.3f}"
    except Exception:
        return str(x)


def main() -> int:
    clips = _load_labels()
    import power

    meta = {
        "generated": time.strftime("%Y-%m-%d %H:%M:%S"),
        "power": power.get_power_status(),
        "ov_cache": str(CACHE_DIR),
    }

    backends = []
    # CUDA baselines
    backends.append(FasterWhisperBackend("medium.en", "cuda", "float16"))
    backends.append(FasterWhisperBackend("small.en", "cuda", "float16"))

    # OpenVINO
    for model_name in ("whisper-base.en-int8", "whisper-small.en-int8"):
        md = _find_ov_model(model_name)
        if not md:
            print(f"MISSING OV model {model_name} under {OV_MODELS}")
            continue
        for device in ("NPU", "CPU", "GPU.0"):
            backends.append(OpenVINOWhisperBackend(md, device))

    # DischargeRate is only meaningful on battery. Skip the 60s waits when AC is
    # connected so we can finish latency/accuracy without unplugging.
    on_ac = bool(meta["power"].get("is_charging") or meta["power"].get("ac_line_status") == 1)
    if on_ac:
        print("AC connected — skipping idle/busy discharge sampling (keep charger plugged).")
        idle_mw, idle_note = None, "skipped: AC connected (DischargeRate unavailable)"
    else:
        print("Sampling idle discharge 60s...")
        idle_mw, idle_note = _sample_discharge(60)

    results: list[BackendResult] = []
    for b in backends:
        print(f"=== {b.name} ===")
        r = evaluate_backend(b, clips, repeats=5)
        r.discharge_idle_mw = idle_mw
        r.power_note = idle_note
        if r.ok and not on_ac:
            print(f"Busy discharge 60s while looping {b.name}...")
            # reload for busy sample
            try:
                b.load()

                def _busy():
                    t_end = time.time() + 60
                    i = 0
                    while time.time() < t_end:
                        clip = clips[i % len(clips)]
                        b.transcribe(ROOT / clip["path"])
                        i += 1

                import threading

                th = threading.Thread(target=_busy, daemon=True)
                th.start()
                busy_mw, busy_note = _sample_discharge(60)
                th.join(timeout=70)
                r.discharge_busy_mw = busy_mw
                r.power_note = f"idle: {idle_note}; busy: {busy_note}"
                b.unload()
            except Exception as e:
                r.power_note += f" | busy sample failed: {e}"
        elif r.ok and on_ac:
            r.power_note = "skipped: AC connected (DischargeRate unavailable)"
        results.append(r)
        print(
            f"  ok={r.ok} p95={r.warm_p95_sec} wer={r.wer} acc={r.command_accuracy} err={bool(r.error)}"
        )

    # Stability on first successful NPU backend
    for r, b in zip(results, backends):
        if r.ok and "NPU" in b.name:
            print(f"Stability 50 on {b.name}")
            r.stability_50 = stability_50(b, clips)
            break

    payload = {"meta": meta, "results": [asdict(r) for r in results]}
    RESULTS_JSON.parent.mkdir(parents=True, exist_ok=True)
    RESULTS_JSON.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    write_md(results, meta)
    print(f"Wrote {RESULTS_JSON} and {RESULTS_MD}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
