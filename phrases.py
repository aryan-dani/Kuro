"""Phrase-first command router — desk power, music, news without the LLM."""

from __future__ import annotations

import logging
import re
import threading
from collections.abc import Callable
from dataclasses import dataclass, field

import music
import tools

logger = logging.getLogger(__name__)

_home_lock = threading.Lock()
_home_inflight = False


@dataclass
class PhraseResult:
    reply: str
    skip_tts: bool = False
    after_speak: Callable[[], None] | None = field(default=None)
    force_fast_tts: bool = False


def _normalize(text: str) -> str:
    text = text.lower().strip()
    replacements = {
        "daddy's": "daddy",
        "daddys": "daddy",
        "dadi": "daddy",
        "wake up": "wakeup",
        "wake-up": "wakeup",
        "woke up": "wakeup",
        "good night": "goodnight",
        "his home": "daddy home",
        "in his home": "daddy home",
        "at his home": "daddy home",
        "in's home": "daddy home",
        "what's": "whats",
        "what s": "whats",
        "moniter": "monitor",
        "monitors": "monitor",
        "extention": "extension",
    }
    for a, b in replacements.items():
        text = text.replace(a, b)
    text = re.sub(r"[^a-z0-9\s]", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    return text


def looks_like_daddys_home(text: str) -> bool:
    return _is_daddys_home(_normalize(text or ""))


def fire_daddys_home_parallel() -> bool:
    """Non-blocking ritual: plug + song. Speak 'Welcome home' immediately in caller."""
    global _home_inflight
    with _home_lock:
        if _home_inflight:
            return False
        _home_inflight = True

    def _plug():
        try:
            r = tools.execute_tool("control_smart_plug", {"scene": "daddy_home"})
            logger.info("Daddy home plug: %s", r)
        except Exception:
            logger.exception("Daddy home plug failed")

    def _song():
        try:
            import time

            time.sleep(0.15)
            r = music.play_should_i_stay_or_should_i_go()
            logger.info("Daddy home song: %s", r)
        except Exception:
            logger.exception("Daddy home song failed")
        finally:
            global _home_inflight
            with _home_lock:
                _home_inflight = False

    threading.Thread(target=_plug, daemon=True, name="KuroHomePlug").start()
    threading.Thread(target=_song, daemon=True, name="KuroHomeSong").start()
    return True


def matches(text: str) -> bool:
    """True if text would route to a known command (no side effects)."""
    if not text or not text.strip():
        return False
    t = _normalize(text)
    return any(
        (
            _is_pause(t),
            _is_resume(t),
            _is_stop_music(t),
            _is_goodnight(t),
            _is_daddys_home(t),
            _is_power_off(t),
            _is_power_on(t),
            _is_news(t),
        )
    )


def try_handle(text: str) -> PhraseResult | None:
    """Route known desk commands. Returns None only when no phrase matched."""
    if not text or not text.strip():
        return None
    t = _normalize(text)
    logger.info("Phrase normalize: %r -> %r", text, t)

    if _is_pause(t):
        return PhraseResult(music.pause_song())

    if _is_resume(t):
        return PhraseResult(music.resume_song())

    if _is_stop_music(t):
        return PhraseResult(music.stop_song())

    if _is_goodnight(t):
        music.stop_song()
        return PhraseResult(_power("good_night", speak_ok="Desk off. Good night, sir."))

    if _is_daddys_home(t):
        started = fire_daddys_home_parallel()
        if not started:
            return PhraseResult("Already on it, sir.")
        return PhraseResult("Welcome home, sir.")

    if _is_power_off(t):
        music.stop_song()
        return PhraseResult(_power("power_off", speak_ok="Desk off."))

    if _is_power_on(t):
        return PhraseResult(_power("power_on", speak_ok="Desk on."))

    if _is_news(t):
        topic = _news_topic(t)
        raw = tools.execute_tool("get_news", {"topic": topic})
        return PhraseResult(_news_to_speech(raw))

    return None


def _power(scene: str, *, speak_ok: str) -> str:
    """Run smart plug scene; speak success short-form or the real error."""
    try:
        raw = tools.execute_tool("control_smart_plug", {"scene": scene})
        logger.info("Power scene %s -> %s", scene, raw)
        if not raw:
            return "I couldn't reach the plug."
        low = raw.lower()
        if low.startswith("success"):
            return speak_ok
        if any(k in low for k in ("can't reach", "unable", "error", "missing", "offline")):
            return raw if len(raw) < 160 else "I couldn't reach the plug."
        return speak_ok
    except Exception:
        logger.exception("power scene %s failed", scene)
        return "I couldn't reach the plug."


def _desk_keywords(t: str) -> bool:
    return any(
        w in t
        for w in (
            "light",
            "lights",
            "lamp",
            "plug",
            "charger",
            "monitor",
            "screen",
            "display",
            "desk",
            "board",
            "extension",
        )
    )


def _is_pause(t: str) -> bool:
    if "pause" not in t:
        return False
    return any(w in t for w in ("song", "music", "track", "audio")) or t in (
        "pause",
        "pause it",
        "pause please",
    )


def _is_resume(t: str) -> bool:
    return any(
        p in t
        for p in (
            "resume",
            "unpause",
            "continue the song",
            "continue music",
            "play the song",
            "play music",
            "keep playing",
        )
    )


def _is_stop_music(t: str) -> bool:
    if not any(w in t for w in ("stop", "close", "shut off", "shut down")):
        return False
    return any(w in t for w in ("song", "music", "track", "clash"))


def _is_goodnight(t: str) -> bool:
    if "goodnight" in t:
        return True
    if "good" in t and "night" in t:
        return True
    return t in ("night", "sleep", "bedtime")


def _is_daddys_home(t: str) -> bool:
    has_home = "home" in t
    has_daddy = "daddy" in t
    has_wake = "wakeup" in t or "wake" in t
    if has_wake and has_home:
        return True
    if has_daddy and has_home:
        return True
    if has_wake and has_daddy:
        return True
    return False


def _is_power_off(t: str) -> bool:
    if any(
        p in t
        for p in (
            "lights off",
            "light off",
            "plug off",
            "switch off",
            "power off",
            "power down",
            "monitor off",
            "screen off",
            "desk off",
            "display off",
        )
    ):
        return True
    if "off" in t and _desk_keywords(t):
        return True
    if "turn off" in t or "switch off" in t:
        return True
    return False


def _is_power_on(t: str) -> bool:
    if "off" in t:
        return False
    if any(
        p in t
        for p in (
            "lights on",
            "light on",
            "plug on",
            "switch on",
            "power on",
            "monitor on",
            "screen on",
            "desk on",
            "display on",
        )
    ):
        return True
    if "on" in t and _desk_keywords(t):
        return True
    if "turn on" in t or "switch on" in t:
        return True
    return False


def _is_news(t: str) -> bool:
    return any(
        p in t
        for p in (
            "news",
            "headlines",
            "whats happening",
            "whats the news",
            "brief me",
            "any updates",
        )
    )


def _news_topic(t: str) -> str:
    for key in ("tech", "world", "business", "ai", "sports"):
        if key in t:
            return key
    return "general"


def _news_to_speech(raw: str) -> str:
    if not raw or "unable" in raw.lower():
        return "I could not fetch the news right now."
    lines = []
    for line in raw.splitlines():
        line = line.strip().lstrip("-").strip()
        if not line or line.lower().startswith("latest"):
            continue
        if "] " in line:
            line = line.split("] ", 1)[-1]
        title = line.split(":", 1)[0].strip() if ":" in line else line
        if title:
            lines.append(title)
        if len(lines) >= 3:
            break
    if not lines:
        return "No headlines came through."
    if len(lines) == 1:
        return f"Top story: {lines[0]}."
    return f"Here are the top headlines. {lines[0]}. Next: {lines[1]}. And: {lines[2]}."
