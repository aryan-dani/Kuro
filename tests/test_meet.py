"""Meet phrase routing. These tests never call Google."""

from __future__ import annotations

import phrases
from tools import meet


def test_edge_opens_a_focused_app_window():
    args = meet._edge_args(r"C:\Edge\msedge.exe", "https://meet.google.com/abc-defg-hij")
    assert args[0].endswith("msedge.exe")
    assert args[1] == "--app=https://meet.google.com/abc-defg-hij"
    assert "--start-maximized" in args
    assert "--new-window" not in args


def test_meet_phrasings():
    samples = [
        ("Make a Google Meet and send the link to Mom", "mom"),
        ("Make a Meet and send it to Mom", "mom"),
        ("Create a Google Meet for Mom", "mom"),
        ("please make a google meet and send the link to mom on whatsapp", "mom"),
        ("Make a Google Meet", ""),
        ("start a meet", ""),
        ("Send the Google mate link to mob.", "mob"),
        ("Send the Google Meet link to Mom", "mom"),
    ]
    for text, name in samples:
        found = meet.match_phrase(text)
        assert found is not None, text
        assert found["contact"] == name
        assert phrases.matches(text)
    assert meet.match_phrase("desk off") is None
    assert meet.match_phrase("send the link to mom on whatsapp") is None


def test_unknown_contact_is_refused(monkeypatch):
    def _forbidden(*_args, **_kwargs):
        raise AssertionError("Google was called")

    monkeypatch.setattr(meet, "_credentials", _forbidden)
    monkeypatch.setattr(meet, "_create_link", _forbidden)
    monkeypatch.setattr(
        "tools.whatsapp.address_book",
        lambda: {"siya": "910000000001"},
    )
    assert meet.run({"contact": "zzzznotaperson"}) == "I don't know that contact."


def test_phrase_routes_without_google(monkeypatch):
    seen: dict = {}

    def _fake_execute(name, args):
        seen["name"] = name
        seen["args"] = args
        if args.get("contact"):
            return "Success: Sent the Meet link to Mom."
        return "Success: The Meet is open."

    monkeypatch.setattr(phrases.tools, "execute_tool", _fake_execute)
    sent = phrases.try_handle("Make a Google Meet and send the link to Mom")
    assert sent is not None
    assert sent.force_fast_tts
    assert sent.reply == "Sent the Meet link to Mom."
    assert seen["name"] == "make_meet"
    assert seen["args"]["contact"] == "mom"

    opened = phrases.try_handle("Make a Google Meet")
    assert opened is not None
    assert opened.force_fast_tts
    assert opened.reply == "The Meet is open."
    assert seen["args"]["contact"] == ""


def test_missing_login_is_spoken(monkeypatch):
    monkeypatch.setattr(meet, "_credentials", lambda: None)

    def _forbidden(*_args, **_kwargs):
        raise AssertionError("Google was called")

    monkeypatch.setattr(meet, "_create_link", _forbidden)
    assert meet.run({"contact": ""}) == "Google Meet is not signed in."
