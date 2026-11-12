"""Remeasure the stages Phase 1 tried to shorten. No plug switching."""

from __future__ import annotations

import os
import sys
import tempfile
import time
import wave
from pathlib import Path

os.environ["KURO_TIMING"] = "1"
ROOT = Path(__file__).resolve().parents[2]
if str(ROOT / "legacy" / "python") not in sys.path:
    sys.path.insert(0, str(ROOT / "legacy" / "python"))
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import timing  # noqa: E402

_csv_name = "timing_tts.csv" if "--tts-only" in sys.argv else "timing_after.csv"
timing.set_path(Path(os.environ.get("LOCALAPPDATA", ".")) / "Kuro" / _csv_name)

LONG = (
    "Here are the top headlines. Copper stays useful in wire and pipe. "
    "Next, the afternoon is mild and clear. And this last line keeps the reply long enough to stream."
)


def _wav(path: Path) -> None:
    with wave.open(str(path), "w") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(16000)
        handle.writeframes(b"\x00\x01" * 32000)


def main() -> None:
    import music
    import tts
    from stt import transcribe, warm_for_policy
    from tools.smart_plug import probe_status

    try:
        music.stop_song()
    except Exception:
        pass
    tts_only = "--tts-only" in sys.argv
    if not tts_only:
        warm_for_policy()
    if not tts.is_ready():
        tts._warm_xtts(blocking=True)

    wav = Path(tempfile.gettempdir()) / "kuro_timing_clip.wav"
    _wav(wav)

    for index, (kind, text) in enumerate(
        (("news", LONG), ("short", "Done, sir."), ("short", "Done, sir."))
    ):
        if index == 2:
            time.sleep(3)
        timing.begin(kind)
        timing.mark("route_done")
        tts.speak(text, fast=False)
        timing.mark("idle")
        timing.finish()
        print("spoke", kind, flush=True)

    if tts_only:
        return

    for _ in range(4):
        timing.begin("stt")
        timing.mark("audio_saved")
        transcribe(str(wav), partial=False)
        timing.mark("stt_done")
        timing.finish()
    print("stt done", flush=True)

    for _ in range(4):
        print("plug", probe_status(), flush=True)


if __name__ == "__main__":
    main()
