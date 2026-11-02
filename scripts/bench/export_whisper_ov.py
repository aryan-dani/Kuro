#!/usr/bin/env python3
"""Export Whisper OpenVINO IR (stateful INT8) for NPU/CPU/iGPU bench."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "bench" / "models"
MODELS = (
    ("openai/whisper-base.en", "whisper-base.en-int8"),
    ("openai/whisper-small.en", "whisper-small.en-int8"),
)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    for hf, name in MODELS:
        dest = OUT / name
        if (dest / "openvino_model.xml").exists() or list(dest.glob("*.xml")):
            print(f"SKIP existing {dest}")
            continue
        cmd = [
            sys.executable,
            "-m",
            "optimum.exporters.openvino",
            "--model",
            hf,
            "--trust-remote-code",
            "--weight-format",
            "int8",
            str(dest),
        ]
        # Fallback CLI name used by docs
        print("RUN:", " ".join(cmd))
        r = subprocess.run(cmd, cwd=str(ROOT))
        if r.returncode != 0:
            cmd2 = [
                "optimum-cli",
                "export",
                "openvino",
                "--trust-remote-code",
                "--model",
                hf,
                str(dest),
                "--weight-format",
                "int8",
            ]
            print("RETRY:", " ".join(cmd2))
            r = subprocess.run(cmd2, cwd=str(ROOT))
        if r.returncode != 0:
            print(f"FAIL export {hf}", file=sys.stderr)
            return r.returncode
        print(f"OK {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
