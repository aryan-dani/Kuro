"""Map transcript to Kuro intent without side effects (no try_handle)."""

from __future__ import annotations

import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

# phrases imports music/tools at module level; stub them for the bench.
for _name in ("music", "tools"):
    if _name not in sys.modules:
        _m = types.ModuleType(_name)
        if _name == "tools":
            _m.execute_tool = lambda *a, **k: ""
            _m.get_tool_definitions = lambda: []
        sys.modules[_name] = _m

import phrases  # noqa: E402


def predict_intent(text: str) -> str:
    if not text or not str(text).strip():
        return "none"
    t = phrases._normalize(text)
    if phrases._is_daddys_home(t):
        return "daddy_home"
    if phrases._is_goodnight(t):
        return "good_night"
    if phrases._is_pause(t):
        return "pause"
    if phrases._is_resume(t):
        return "resume"
    if phrases._is_stop_music(t):
        return "stop_music"
    if phrases._is_inbox(t):
        return "inbox"
    if phrases._is_news(t):
        return "news"
    if phrases._is_power_off(t):
        return "power_off"
    if phrases._is_power_on(t):
        return "power_on"
    return "none"
