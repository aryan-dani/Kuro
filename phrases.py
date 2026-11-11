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
        # ASR often drops the second f: "monitor of" / "lights of"
        "monitor of": "monitor off",
        "screen of": "screen off",
        "display of": "display off",
        "desk of": "desk off",
        "lights of": "lights off",
        "light of": "light off",
        "plug of": "plug off",
        "power of": "power off",
        "board of": "board off",
        # Harmless ASR garble that still ends in off (never maps off→on)
        "let's off": "desk off",
        "lets off": "desk off",
        "let s off": "desk off",
        "next off": "desk off",
        "bob it off": "power off",
        "urgent man": "urgent mail",
        "urgent men": "urgent mail",
    }
    for a, b in replacements.items():
        text = text.replace(a, b)
    text = re.sub(r"[^a-z0-9\s]", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    # Second pass after punctuation strip (ASR often emits Let's Off!)
    post = {
        "lets off": "desk off",
        "let s off": "desk off",
        "next off": "desk off",
        "bob it off": "power off",
    }
    for a, b in post.items():
        if a in text:
            text = text.replace(a, b)
    return text


def _has_word(t: str, word: str) -> bool:
    """Whole-word match so 'on' does not hit inside 'monitor'."""
    return re.search(rf"\b{re.escape(word)}\b", t) is not None


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
            _is_cancel(t),
            _is_thanks(t),
            _is_help(t),
            _is_identity(t),
            _is_time(t),
            _is_date(t),
            _is_pause(t),
            _is_resume(t),
            _is_stop_music(t),
            _is_goodnight(t),
            _is_daddys_home(t),
            _is_power_off(t),
            _is_power_on(t),
            _is_news(t),
            _is_inbox(t),
            _is_meet(t),
            _is_whatsapp(t),
            _is_workflow(t),
            _is_workflow_hint(t),
        )
    )


def try_handle(text: str) -> PhraseResult | None:
    """Route known desk commands. Returns None only when no phrase matched."""
    if not text or not text.strip():
        return None
    t = _normalize(text)
    logger.info("Phrase normalize: %r -> %r", text, t)

    # Tiny local replies first — never touch hub, n8n, or Ollama.
    if _is_cancel(t):
        return PhraseResult("Alright.", force_fast_tts=True)

    if _is_thanks(t):
        return PhraseResult("Anytime.", force_fast_tts=True)

    if _is_help(t):
        return PhraseResult(
            "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. "
            "Say run hello workflow to test n8n.",
            force_fast_tts=True,
        )

    if _is_identity(t):
        return PhraseResult("Kuro. Local assistant. Phrases first, model last.", force_fast_tts=True)

    if _is_time(t):
        return PhraseResult(_clock_reply(), force_fast_tts=True)

    if _is_date(t):
        return PhraseResult(_date_reply(), force_fast_tts=True)

    if _is_pause(t):
        return PhraseResult(music.pause_song(), force_fast_tts=True)

    if _is_resume(t):
        return PhraseResult(music.resume_song(), force_fast_tts=True)

    if _is_stop_music(t):
        return PhraseResult(music.stop_song(), force_fast_tts=True)

    if _is_goodnight(t):
        music.stop_song()
        return PhraseResult(
            _power("good_night", speak_ok="Desk off. Good night, sir."),
            force_fast_tts=True,
        )

    if _is_daddys_home(t):
        started = fire_daddys_home_parallel()
        if not started:
            return PhraseResult("Already on it, sir.", force_fast_tts=True)
        return PhraseResult("Welcome home, sir.", force_fast_tts=True)

    if _is_power_off(t):
        music.stop_song()
        return PhraseResult(_power("power_off", speak_ok="Desk off."), force_fast_tts=True)

    if _is_power_on(t):
        return PhraseResult(_power("power_on", speak_ok="Desk on."), force_fast_tts=True)

    if _is_news(t):
        topic = _news_topic(t)
        raw = tools.execute_tool("get_news", {"topic": topic})
        return PhraseResult(_news_to_speech(raw), force_fast_tts=True)

    if _is_meet(t):
        return PhraseResult(_meet_reply(t), force_fast_tts=True)

    if _is_inbox(t):
        mode = _inbox_mode(t)
        raw = tools.execute_tool("check_inbox", {"mode": mode})
        return PhraseResult(_inbox_to_speech(raw), force_fast_tts=True)

    if _is_whatsapp(t):
        return PhraseResult(_whatsapp_reply(t), force_fast_tts=True)

    if _is_workflow(t):
        return PhraseResult(_workflow_reply(t), force_fast_tts=True)

    # User clearly asked for a workflow but the name did not match — do not escalate to LLM.
    if _is_workflow_hint(t):
        return PhraseResult(
            "I don't know that workflow. Check the local n8n registry.",
            force_fast_tts=True,
        )

    return None


def _is_cancel(t: str) -> bool:
    return t in (
        "cancel",
        "never mind",
        "nevermind",
        "forget it",
        "stop",
        "abort",
    )


def _is_thanks(t: str) -> bool:
    return any(p in t for p in ("thank you", "thanks", "cheers", "appreciate it"))


def _is_help(t: str) -> bool:
    return any(
        p in t
        for p in (
            "what can you do",
            "help me",
            "your commands",
            "what do you support",
            "list commands",
            "capabilities",
        )
    ) or t in ("help", "commands")


def _is_identity(t: str) -> bool:
    return any(
        p in t
        for p in (
            "who are you",
            "what are you",
            "your name",
            "whats your name",
        )
    )


def _is_time(t: str) -> bool:
    if _is_date(t) and "time" not in t:
        return False
    return any(
        p in t
        for p in (
            "what time",
            "whats the time",
            "current time",
            "tell me the time",
            "time is it",
        )
    ) or t in ("time", "the time")


def _is_date(t: str) -> bool:
    return any(
        p in t
        for p in (
            "what day",
            "whats the date",
            "what date",
            "todays date",
            "today's date",
            "current date",
            "what is today",
        )
    ) or t in ("date", "the date")


def _clock_reply() -> str:
    from datetime import datetime

    now = datetime.now()
    return now.strftime("It's %I:%M %p.").lstrip("0").replace(" 0", " ")


def _date_reply() -> str:
    from datetime import datetime

    now = datetime.now()
    return now.strftime("Today is %A, %B %d.").replace(" 0", " ")


def _is_workflow_hint(t: str) -> bool:
    if "n8n" in t or "workflow" in t:
        return True
    return "automation" in t and any(v in t for v in ("run", "start", "trigger", "launch"))


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
    # Whole words. "board" must not match inside "clipboard".
    return any(
        _has_word(t, w)
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
    return any(w in t for w in ("song", "music", "track", "audio", "media")) or t in (
        "pause",
        "pause it",
        "pause please",
        "pause that",
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
            "resume music",
            "resume song",
        )
    )


def _is_stop_music(t: str) -> bool:
    if not any(w in t for w in ("stop", "close", "shut off", "shut down", "kill")):
        return False
    return any(w in t for w in ("song", "music", "track", "clash", "media"))


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
    # Prefer explicit off phrases; never treat "of"/"on" as off here.
    if _has_word(t, "on") and not _has_word(t, "off"):
        return False
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
    if _has_word(t, "off") and _desk_keywords(t):
        return True
    if "turn off" in t or "switch off" in t:
        return True
    return False


def _is_power_on(t: str) -> bool:
    # Whole-word "off" wins — never promote an off command to on.
    if _has_word(t, "off"):
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
    # Word-boundary "on" only — substring match falsely hit "monitor".
    if _has_word(t, "on") and _desk_keywords(t):
        return True
    if "turn on" in t or "switch on" in t:
        return True
    return False


def _is_news(t: str) -> bool:
    # Avoid stealing inbox queries that mention "updates" about mail
    if _is_inbox(t):
        return False
    return any(
        p in t
        for p in (
            "news",
            "headlines",
            "whats happening",
            "whats the news",
            "what is the news",
            "brief me",
            "any updates",
            "latest news",
            "give me the news",
            "read the news",
        )
    )


def _is_inbox(t: str) -> bool:
    return any(
        p in t
        for p in (
            "urgent mail",
            "urgent email",
            "any urgent",
            "inbox check",
            "check inbox",
            "check my inbox",
            "check my email",
            "check my mail",
            "check email",
            "check mail",
            "deadlines today",
            "any deadlines",
            "my deadlines",
            "important email",
            "important mail",
            "important emails",
            "missed email",
            "missed mail",
        )
    )


def _is_meet(t: str) -> bool:
    try:
        from tools.meet import match_phrase

        return match_phrase(t) is not None
    except Exception:
        logger.exception("Meet phrase match failed")
        return False


def _meet_reply(t: str) -> str:
    try:
        from tools.meet import match_phrase

        found = match_phrase(t) or {}
        raw = tools.execute_tool("make_meet", {"contact": found.get("contact") or ""})
        return _speakable(raw)
    except Exception:
        logger.exception("Meet phrase failed")
        return "I couldn't create that Google Meet."


def _is_whatsapp(t: str) -> bool:
    try:
        from tools.whatsapp import match_phrase

        return match_phrase(t) is not None
    except Exception:
        logger.exception("WhatsApp phrase match failed")
        return False


def _is_workflow(t: str) -> bool:
    try:
        from tools.n8n import match_phrase

        return match_phrase(t) is not None
    except Exception:
        logger.exception("Workflow phrase match failed")
        return False


def _speakable(raw: str) -> str:
    text = (raw or "").strip()
    if text.lower().startswith("success:"):
        text = text.split(":", 1)[1].strip()
    return text or "Done, sir."


def _whatsapp_reply(t: str) -> str:
    try:
        from tools.whatsapp import match_phrase

        found = match_phrase(t)
        if not found:
            return "I don't know who to message."
        raw = tools.execute_tool(
            "send_whatsapp",
            {"contact": found["contact"], "source": found["source"], "text": ""},
        )
        return _speakable(raw)
    except Exception:
        logger.exception("WhatsApp phrase failed")
        return "I couldn't send that WhatsApp."


def _workflow_reply(t: str) -> str:
    try:
        from tools.n8n import match_phrase

        found = match_phrase(t)
        if not found:
            return "I don't know that workflow."
        wf, extra = found
        raw = tools.execute_tool(
            "run_workflow",
            {"workflow": wf["id"], "input": extra},
        )
        return _speakable(raw)
    except Exception:
        logger.exception("Workflow phrase failed")
        return "I couldn't run that workflow."


def _inbox_mode(t: str) -> str:
    if "deadline" in t:
        return "deadlines"
    if "urgent" in t or "important" in t:
        return "urgent"
    return "summary"


def _inbox_to_speech(raw: str) -> str:
    if not raw or not str(raw).strip():
        return "I couldn't reach the cloud inbox."
    text = str(raw).strip()
    if len(text) > 280:
        return text[:277].rstrip() + "..."
    return text


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
