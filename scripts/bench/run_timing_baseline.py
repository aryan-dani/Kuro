"""Run 20 real command stages into the timing CSV.

Voice rows transcribe one short clip, then run the labeled command.
The desk plug is only asked for status. It is not switched.
"""

from __future__ import annotations

import os
import sys
import tempfile
import wave
from pathlib import Path

os.environ["KURO_TIMING"] = "1"

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import timing  # noqa: E402


def _speech_wav(path: Path) -> None:
    import pyttsx3

    engine = pyttsx3.init()
    engine.save_to_file("What is the news today.", str(path))
    engine.runAndWait()
    try:
        engine.stop()
    except Exception:
        pass


def _silence_wav(path: Path) -> None:
    with wave.open(str(path), "w") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(16000)
        handle.writeframes(b"\x00\x00" * 16000)


def _voice(kind: str, text: str, wav: Path, route) -> None:
    import tts
    from stt import transcribe

    timing.begin(kind)
    timing.mark("hotkey_down")
    timing.mark("recording_start")
    timing.mark("hotkey_up")
    timing.mark("audio_saved")
    transcribe(str(wav), partial=False)
    timing.mark("stt_done")
    reply = route(text)
    timing.mark("route_done")
    if reply:
        tts.speak(reply, fast=True)
    timing.mark("idle")
    timing.finish()


def _music(text: str) -> str:
    import phrases

    found = phrases.try_handle(text)
    return found.reply if found else ""


def _news(text: str) -> str:
    import phrases

    found = phrases.try_handle(text)
    return found.reply if found else ""


def _llm(text: str) -> str:
    import brain

    return brain.think(text)


def main() -> None:
    import stt
    import tts
    from tools.smart_plug import probe_status

    started = timing.tick()
    label = stt.warm_for_policy()
    timing.once("startup", "whisper_load", timing.since(started))
    print("whisper", label, flush=True)

    import compute_policy

    if tts.is_ready():
        print("xtts already ready", flush=True)
    elif compute_policy.ready_for_heavy():
        started = timing.tick()
        try:
            tts._warm_xtts(blocking=True)
            print("xtts warmed", flush=True)
        except Exception:
            print("xtts not warmed", flush=True)
        timing.once("startup", "xtts_load", timing.since(started))
    else:
        print("xtts left cold", flush=True)

    folder = Path(tempfile.mkdtemp(prefix="kuro-timing-"))
    wav = folder / "clip.wav"
    try:
        _speech_wav(wav)
    except Exception:
        _silence_wav(wav)
    if not wav.is_file() or wav.stat().st_size < 1000:
        _silence_wav(wav)

    jobs = (
        [("music", "pause the song", _music)] * 3
        + [("music", "resume the song", _music)] * 3
        + [("news", "what's the news today", _news)] * 6
        + [("llm", "Tell me one plain fact about copper.", _llm)] * 4
    )
    for index, (kind, text, route) in enumerate(jobs, start=1):
        print(f"command {index} {kind}", flush=True)
        try:
            _voice(kind, text, wav, route)
        except Exception as exc:
            print(f"command {index} failed {type(exc).__name__}", flush=True)
            timing.mark("idle")
            timing.finish()

    for index in range(4):
        print(f"plug status {index + 1}", flush=True)
        state = probe_status()
        print(f"plug {state}", flush=True)

    print(f"csv {timing.csv_path()}", flush=True)


if __name__ == "__main__":
    main()
