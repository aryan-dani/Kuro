"""Model context stays until the conversation is cleared."""

from __future__ import annotations

import brain


def _brain() -> brain.Brain:
    instance = brain.Brain.__new__(brain.Brain)
    instance._messages = []
    instance._updated = 0.0
    return instance


def test_follow_up_keeps_earlier_turns():
    instance = _brain()
    instance._thread("one")
    instance._remember("ack")
    instance._updated = 0.0
    messages = instance._thread("two")
    contents = [item.get("content") for item in messages]
    assert "one" in contents
    assert "ack" in contents
    assert contents[-1] == "two"


def test_thread_caps_at_twenty_turns_and_clear_drops_it():
    instance = _brain()
    for index in range(25):
        instance._thread(f"u{index}")
        instance._remember(f"a{index}")
    rest = instance._messages[1:]
    assert len(rest) <= 40
    assert rest[-2]["content"] == "u24"
    assert rest[-1]["content"] == "a24"
    assert all(item.get("content") != "u0" for item in rest)
    instance.clear()
    assert instance._messages == []
