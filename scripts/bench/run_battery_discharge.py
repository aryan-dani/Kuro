#!/usr/bin/env python3
"""Battery-only discharge comparison: idle / NPU small / CUDA small.

Requires AC unplugged. Refuses to run while charging.

Each active run: 20 minutes, one command transcription every 30s.
Logs WMI BatteryStatus.DischargeRate every 5s.
Reports mean discharge mW; flags if NPU saves <10% vs CUDA.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "legacy" / "python"))
sys.path.insert(0, str(ROOT))

import power  # noqa: E402

OUT = ROOT / "bench" / "battery_discharge.json"
WAVS = ROOT / "bench" / "data" / "wavs"
DURATION_SEC = 20 * 60
CMD_EVERY = 30
SAMPLE_EVERY = 5


def _discharge_mw() -> float | None:
    ps = (
        "Get-CimInstance -Namespace root/wmi -ClassName BatteryStatus "
        "| Select-Object -ExpandProperty DischargeRate"
    )
    try:
        out = subprocess.check_output(
            ["powershell", "-NoProfile", "-Command", ps],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
        if not out:
            return None
        return float(out.splitlines()[-1].strip())
    except Exception:
        return None


def _require_battery() -> dict:
    st = power.get_power_status()
    if st.get("is_charging") or st.get("ac_line_status") == 1:
        raise SystemExit(
            "REFUSE: laptop is on AC. Unplug the charger, then re-run this script.\n"
            f"power={st}"
        )
    return st


def _cmd_wavs() -> list[Path]:
    paths = sorted(WAVS.glob("cmd_*.wav"))
    if not paths:
        raise SystemExit(f"No cmd_*.wav under {WAVS}")
    return paths


def run_idle(seconds: int) -> dict:
    print(f"IDLE {seconds}s — models unloaded")
    samples = []
    t0 = time.time()
    while time.time() - t0 < seconds:
        r = _discharge_mw()
        if r is not None and r > 0:
            samples.append({"t": time.time() - t0, "mw": r})
        time.sleep(SAMPLE_EVERY)
    vals = [s["mw"] for s in samples]
    return {
        "name": "idle",
        "seconds": seconds,
        "mean_mw": statistics.mean(vals) if vals else None,
        "n_samples": len(vals),
        "samples": samples,
    }


def run_backend(name: str, seconds: int) -> dict:
    import stt

    os.environ["KURO_STT_BACKEND"] = "npu" if name == "npu" else "cuda"
    # Force config reload semantics via env; stt.resolve_backend reads env/config
    import importlib

    import config

    importlib.reload(config)
    importlib.reload(stt)

    wavs = _cmd_wavs()
    print(f"{name.upper()} load…")
    # force load
    stt.transcribe(str(wavs[0]), partial=False)

    samples = []
    t0 = time.time()
    next_cmd = t0
    i = 0
    while time.time() - t0 < seconds:
        now = time.time()
        if now >= next_cmd:
            path = wavs[i % len(wavs)]
            stt.transcribe(str(path), partial=False)
            i += 1
            next_cmd = now + CMD_EVERY
        r = _discharge_mw()
        if r is not None and r > 0:
            samples.append({"t": now - t0, "mw": r})
        time.sleep(SAMPLE_EVERY)
    vals = [s["mw"] for s in samples]
    # unload
    with stt._stt_lock:
        stt._unload_cuda()
        stt._unload_npu()
        stt._unload_cpu()
    return {
        "name": name,
        "seconds": seconds,
        "mean_mw": statistics.mean(vals) if vals else None,
        "n_samples": len(vals),
        "commands": i,
        "samples": samples,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=int, default=DURATION_SEC, help="per-run duration")
    ap.add_argument("--skip-idle", action="store_true")
    ap.add_argument("--only", choices=("idle", "npu", "cuda"), default=None)
    args = ap.parse_args()

    meta = {"started": time.strftime("%Y-%m-%d %H:%M:%S"), "power": _require_battery()}
    print("Battery OK:", meta["power"])
    results = []

    order = ["idle", "npu", "cuda"]
    if args.only:
        order = [args.only]
    elif args.skip_idle:
        order = ["npu", "cuda"]

    for name in order:
        _require_battery()
        if name == "idle":
            results.append(run_idle(args.seconds))
        else:
            results.append(run_backend(name, args.seconds))
        print(f"  {name} mean_mw={results[-1].get('mean_mw')}")

    by = {r["name"]: r for r in results}
    note = ""
    if by.get("npu", {}).get("mean_mw") and by.get("cuda", {}).get("mean_mw"):
        npu_m = by["npu"]["mean_mw"]
        cuda_m = by["cuda"]["mean_mw"]
        if cuda_m > 0:
            savings = (cuda_m - npu_m) / cuda_m * 100.0
            if savings < 10.0:
                note = (
                    f"NPU saves {savings:.1f}% vs CUDA — less than 10%. "
                    "Say plainly: NPU power win is under the 10% bar."
                )
            else:
                note = f"NPU saves {savings:.1f}% vs CUDA (>= 10%)."
    payload = {"meta": meta, "results": results, "verdict": note}
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(note or "(incomplete — need both npu and cuda means)")
    print("Wrote", OUT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
