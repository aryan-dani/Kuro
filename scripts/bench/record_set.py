#!/usr/bin/env python3
"""Interactive 16 kHz mono clip capture for Kuro STT bench.

Reuses mic selection from main.py. Writes WAV files and bench/data/labels.json.
Never calls phrases.try_handle (no side effects on lights/music).
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import numpy as np
import sounddevice as sd
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

SAMPLE_RATE = 16000
CHANNELS = 1
DATA_DIR = ROOT / "bench" / "data"
WAV_DIR = DATA_DIR / "wavs"


def _pick_mic_device() -> int | None:
    """Prefer onboard Realtek mic array over Bluetooth headsets (from main.py)."""
    try:
        devices = sd.query_devices()
        default_in = sd.default.device[0] if isinstance(sd.default.device, (list, tuple)) else sd.default.device
        preferred = []
        for i, d in enumerate(devices):
            if d.get("max_input_channels", 0) <= 0:
                continue
            name = (d.get("name") or "").lower()
            if "realtek" in name and "mic" in name:
                preferred.append((0, i, d["name"]))
            elif "microphone array" in name:
                preferred.append((1, i, d["name"]))
            elif "microphone" in name and "headset" not in name and "hands-free" not in name:
                preferred.append((2, i, d["name"]))
        if preferred:
            preferred.sort()
            idx = preferred[0][1]
            print(f"Mic: [{idx}] {preferred[0][2]} (default was {default_in})")
            return idx
        print(f"Mic: system default ({default_in})")
    except Exception as e:
        print(f"Mic probe failed: {e}")
    return None


# (id, prompt_to_speak, expected_text, expected_intent, seconds)
CLIPS: list[tuple[str, str, str, str, float]] = [
    ("cmd_daddys_home", "Wake up daddy's home", "wake up daddy's home", "daddy_home", 4.0),
    ("cmd_goodnight", "Good night", "good night", "good_night", 3.0),
    ("cmd_pause", "Pause the song", "pause the song", "pause", 3.0),
    ("cmd_resume", "Resume the song", "resume the song", "resume", 3.0),
    ("cmd_stop", "Stop the music", "stop the music", "stop_music", 3.0),
    ("cmd_news", "What's the news today", "what's the news today", "news", 4.0),
    ("cmd_lights_on", "Turn on the lights", "turn on the lights", "power_on", 3.0),
    ("cmd_lights_off", "Turn off the lights", "turn off the lights", "power_off", 3.0),
    ("cmd_switch_off", "Switch the lights off", "switch the lights off", "power_off", 3.0),
    ("cmd_monitor_off", "Monitor off", "monitor off", "power_off", 2.5),
    ("cmd_monitor_on", "Monitor on", "monitor on", "power_on", 2.5),
    ("cmd_desk_off", "Desk off", "desk off", "power_off", 2.5),
    ("cmd_desk_on", "Desk on", "desk on", "power_on", 2.5),
    ("cmd_power_on", "Power on", "power on", "power_on", 2.5),
    ("cmd_power_off", "Power off", "power off", "power_off", 2.5),
    ("cmd_hello", "Hello Kuro", "hello kuro", "none", 2.5),
    ("cmd_urgent_mail", "Any urgent mail", "any urgent mail", "inbox", 3.0),
    ("cmd_inbox_check", "Inbox check", "inbox check", "inbox", 2.5),
    ("cmd_deadlines", "Deadlines today", "deadlines today", "inbox", 3.0),
    ("free_1", "What time is it in Tokyo", "what time is it in tokyo", "none", 4.0),
    ("free_2", "Remind me to call mom later", "remind me to call mom later", "none", 4.0),
    ("free_3", "How is the weather this weekend", "how is the weather this weekend", "none", 4.0),
    ("free_4", "Summarize my afternoon schedule", "summarize my afternoon schedule", "none", 4.0),
    ("free_5", "Is the kitchen light still on", "is the kitchen light still on", "none", 4.0),
    ("free_6", "Play something calm while I work", "play something calm while i work", "none", 4.0),
    ("noise_quiet", "Speak softly: lights off", "lights off", "power_off", 4.0),
    ("noise_far", "Speak from farther away: pause the song", "pause the song", "pause", 4.0),
    ("noise_fast", "Speak quickly: desk on", "desk on", "power_on", 3.0),
    (
        "dictation_1",
        "Read naturally for about 15 seconds about your day so far, any topic",
        "",
        "dictation",
        18.0,
    ),
    (
        "dictation_2",
        "Read naturally for about 15 seconds describing your desk setup",
        "",
        "dictation",
        18.0,
    ),
]


def _record(path: Path, seconds: float, device: int | None) -> None:
    frames = int(seconds * SAMPLE_RATE)
    print(f"  Recording {seconds:.1f}s ...")
    audio = sd.rec(
        frames,
        samplerate=SAMPLE_RATE,
        channels=CHANNELS,
        dtype="float32",
        device=device,
    )
    sd.wait()
    # Soft peak normalize without amplifying silence hard
    peak = float(np.max(np.abs(audio))) if audio.size else 0.0
    if peak > 1e-4:
        audio = audio / peak * 0.9
    sf.write(str(path), audio, SAMPLE_RATE, subtype="PCM_16")
    print(f"  Saved {path.name} (peak={peak:.3f})")


def main() -> int:
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    WAV_DIR.mkdir(parents=True, exist_ok=True)
    device = _pick_mic_device()

    print()
    print(f"Will capture {len(CLIPS)} clips into {WAV_DIR}")
    print("Press Enter when ready for each prompt. Ctrl+C to abort.")
    print()

    labels: list[dict] = []
    try:
        for i, (cid, prompt, expected, intent, seconds) in enumerate(CLIPS, 1):
            wav_path = WAV_DIR / f"{cid}.wav"
            print(f"[{i}/{len(CLIPS)}] {cid}")
            print(f"  SAY: {prompt}")
            input("  Press Enter, then speak ... ")
            time.sleep(0.25)
            _record(wav_path, seconds, device)
            labels.append(
                {
                    "id": cid,
                    "path": str(wav_path.relative_to(ROOT)).replace("\\", "/"),
                    "prompt": prompt,
                    "expected_text": expected,
                    "expected_intent": intent,
                    "duration_sec": seconds,
                }
            )
            print()
    except KeyboardInterrupt:
        print("\nAborted. Writing partial labels.")

    out = DATA_DIR / "labels.json"
    payload = {"sample_rate": SAMPLE_RATE, "channels": CHANNELS, "clips": labels}
    out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(f"Wrote {out} ({len(labels)} clips)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
