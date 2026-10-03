"""Reliable local music playback for Kuro (play / pause / resume / stop)."""

from __future__ import annotations

import logging
import os
import threading
import time
from pathlib import Path

logger = logging.getLogger(__name__)

SONG_CANDIDATES = (
    "should_i_stay_or_should_i_go.mp3",
    "should_i_stay_or_should_i_go.wav",
    "Should I Stay or Should I Go.mp3",
)

YOUTUBE_URL = "https://www.youtube.com/watch?v=BNMSWlvom4s"

_lock = threading.RLock()
_mixer_ready = False
_paused = False
_current_path: Path | None = None


def _ensure_mixer() -> bool:
    global _mixer_ready
    with _lock:
        if _mixer_ready:
            return True
        try:
            import pygame

            # Explicit device init — frequency matches most MP3s
            pygame.mixer.pre_init(frequency=44100, size=-16, channels=2, buffer=2048)
            pygame.mixer.init()
            _mixer_ready = True
            logger.info("pygame mixer ready")
            return True
        except Exception as e:
            logger.error("pygame mixer init failed: %s", e)
            return False


def find_local_song() -> Path | None:
    env_path = os.getenv("DADDY_HOME_SONG_PATH", "").strip().strip('"')
    if env_path:
        p = Path(env_path)
        if p.is_file():
            return p.resolve()
    root = Path(__file__).resolve().parent
    for folder in (root / "sounds", root / "music", root):
        if not folder.is_dir():
            continue
        for name in SONG_CANDIDATES:
            candidate = folder / name
            if candidate.is_file():
                return candidate.resolve()
    return None


def is_busy() -> bool:
    with _lock:
        if not _ensure_mixer():
            return False
        import pygame

        return bool(pygame.mixer.music.get_busy() or _paused)


def is_paused() -> bool:
    with _lock:
        return _paused


def stop_song() -> str:
    global _paused, _current_path
    with _lock:
        _paused = False
        _current_path = None
        try:
            if _ensure_mixer():
                import pygame

                pygame.mixer.music.stop()
        except Exception as e:
            logger.warning("stop_song: %s", e)
        try:
            import sounddevice as sd

            sd.stop()
        except Exception:
            pass
    return "Music stopped."


def pause_song() -> str:
    global _paused
    with _lock:
        if not _ensure_mixer():
            return "Music player is unavailable."
        import pygame

        if not pygame.mixer.music.get_busy() and not _paused:
            return "Nothing is playing."
        pygame.mixer.music.pause()
        _paused = True
        logger.info("Music paused")
        return "Paused."


def resume_song() -> str:
    global _paused
    with _lock:
        if not _ensure_mixer():
            return "Music player is unavailable."
        import pygame

        if not _paused and not pygame.mixer.music.get_busy():
            # Nothing paused — try replay current / default track
            path = _current_path or find_local_song()
            if path:
                return play_file(path)
            return "Nothing to resume."
        pygame.mixer.music.unpause()
        _paused = False
        logger.info("Music resumed")
        return "Resumed."


def play_file(path: Path, retries: int = 3) -> str:
    """Play an audio file with retries. Stops any current track first."""
    global _paused, _current_path
    path = Path(path).resolve()
    if not path.is_file():
        return f"File not found: {path}"

    with _lock:
        if not _ensure_mixer():
            return "Music player failed to start."
        import pygame

        last_err = None
        for attempt in range(1, retries + 1):
            try:
                # Stop everything that might own the device
                try:
                    import sounddevice as sd

                    sd.stop()
                except Exception:
                    pass
                pygame.mixer.music.stop()
                time.sleep(0.05)
                pygame.mixer.music.load(str(path))
                pygame.mixer.music.set_volume(1.0)
                pygame.mixer.music.play()
                # Confirm it actually started
                time.sleep(0.15)
                if pygame.mixer.music.get_busy():
                    _paused = False
                    _current_path = path
                    logger.info("Playing (%s/%s): %s", attempt, retries, path.name)
                    return f"Playing {path.name}"
                last_err = "mixer reported not busy after play()"
                logger.warning("Play attempt %s failed: %s", attempt, last_err)
                # Re-init mixer and retry
                try:
                    pygame.mixer.quit()
                except Exception:
                    pass
                global _mixer_ready
                _mixer_ready = False
                _ensure_mixer()
            except Exception as e:
                last_err = str(e)
                logger.warning("Play attempt %s error: %s", attempt, e)
                time.sleep(0.1)

    return f"Could not start music ({last_err})."


def play_should_i_stay_or_should_i_go() -> str:
    local = find_local_song()
    if local is None:
        logger.warning("No local MP3 — cannot play song")
        return "Song file is missing from the sounds folder."
    return play_file(local)


def duck_for_speech() -> None:
    """Pause music so Kuro's voice is audible."""
    with _lock:
        if not _ensure_mixer():
            return
        import pygame

        if pygame.mixer.music.get_busy():
            pygame.mixer.music.pause()
            global _paused
            _paused = True
            logger.info("Ducked music for speech")
