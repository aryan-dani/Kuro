"""Kuro entrypoint — acquire singleton BEFORE importing torch/STT/TTS."""

from __future__ import annotations

import time

_LAUNCH = time.perf_counter()

import singleton

if not singleton.acquire():
    singleton.refuse_and_exit()

# Safe to load the heavy stack now — we own the mutex
import main as kuro_main

kuro_main._BOOT_TICK = _LAUNCH
kuro_main.main()
