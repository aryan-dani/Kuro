"""Contact matching, URL check, and WhatsApp phrase routing."""

from __future__ import annotations

import sqlite3

import phrases
from overlay import voice_level
from tools import whatsapp


def test_spoken_recipient_and_focus_guard():
    assert whatsapp.spoken_recipient("me") == "you"
    assert whatsapp.spoken_recipient("Me") == "you"
    assert whatsapp.spoken_recipient("siya") == "Siya"
    assert whatsapp.target_is_front(10, 10)
    assert not whatsapp.target_is_front(10, 11)
    assert not whatsapp.target_is_front(0, 10)
    assert whatsapp._is_whatsapp_title("WhatsApp")
    assert whatsapp._is_whatsapp_title("(2) WhatsApp")
    assert not whatsapp._is_whatsapp_title("WhatsApp Web - Google Chrome")
    assert not whatsapp._is_whatsapp_title("")


def test_clipboard_payload_kind():
    assert whatsapp.payload_kind("hello", set()) == "text"
    assert whatsapp.payload_kind("", {2, 8}) == "image"
    assert whatsapp.payload_kind("", {13}) == "empty"
    assert whatsapp.payload_kind("  ", set()) == "empty"
    assert whatsapp.clean_digits("+91 00000-00000") == "910000000000"
    assert whatsapp.clean_digits("no digits") == ""
    assert whatsapp.url_ok("https://meet.google.com/abc-defg-hij")
    assert whatsapp.url_ok("http://example.com/a")
    assert not whatsapp.url_ok("not a url")
    assert not whatsapp.url_ok("ftp://example.com")
    assert not whatsapp.url_ok("")


def test_one_letter_mishear():
    book = {"Mom": "910000000009", "Bob": "910000000008", "Siya": "910000000001"}
    assert whatsapp.resolve_contact("mob", book)[0] == "Mom"
    assert whatsapp.resolve_contact("mom", book)[0] == "Mom"
    assert whatsapp.resolve_contact("zzzznotaperson", book) is None
    tied = {"Abx": "910000000003", "Aby": "910000000004"}
    assert whatsapp.resolve_contact("abc", tied) is None
    assert whatsapp.resolve_contact("abc", {"Abx": "910000000003"})[0] == "Abx"


def test_contact_match():
    book = {"siya": "910000000001", "aryan": "910000000002"}
    assert whatsapp.resolve_contact("siya", book)[0] == "siya"
    assert whatsapp.resolve_contact("Siya", book)[0] == "siya"
    assert whatsapp.resolve_contact("siyaa", book)[0] == "siya"
    assert whatsapp.resolve_contact("somebody else", book) is None
    assert whatsapp.resolve_contact("zzzzzzzz", book) is None


def test_whatsapp_phrasings():
    samples = [
        ("send this link to siya on whatsapp", "browser_url", "siya"),
        ("send the meeting link to siya on whatsapp", "browser_url", "siya"),
        ("send my clipboard to siya via whatsapp", "clipboard", "siya"),
        ("please send the link to siya in whatsapp", "browser_url", "siya"),
        ("whatsapp siya this link", "browser_url", "siya"),
        ("whatsapp siya the link", "browser_url", "siya"),
        ("hey kuro send this link to siya on whatsapp", "browser_url", "siya"),
        ("can you send clipboard to aryan on whatsapp", "clipboard", "aryan"),
        ("send the message to siya on whatsapp", "clipboard", "siya"),
        ("send me my clipboard on whatsapp", "clipboard", "me"),
        ("send me the link on whatsapp", "browser_url", "me"),
        ("send my clipboard on whatsapp", "clipboard", "me"),
        ("send the link on whatsapp", "browser_url", "me"),
        ("send clipboard on whatsapp", "clipboard", "me"),
        ("send my clipboard history on whatsapp", "clipboard", "me"),
        ("send me my clipboard history on whatsapp", "clipboard", "me"),
    ]
    for text, source, name in samples:
        found = whatsapp.match_phrase(text)
        assert found is not None, text
        assert found["source"] == source
        assert found["contact"] == name
    assert whatsapp.match_phrase("desk off") is None
    assert whatsapp.match_phrase("send this link to siya") is None


def test_clipboard_is_not_desk_power():
    t = phrases._normalize("Send my clipboard to me on WhatsApp.")
    assert phrases._is_whatsapp(t)
    assert not phrases._is_power_on(t)
    assert not phrases._is_power_off(t)


def test_phrases_matches_whatsapp():
    assert phrases.matches("send this link to siya on whatsapp")
    assert phrases.matches("whatsapp siya the link")
    assert phrases.matches("pause the song")
    assert not phrases.matches("hello there")


def test_voice_level_lifts_quiet_speech():
    quiet = [1800] * 200
    silence = [40] * 200
    loud = [12000] * 200
    assert voice_level(quiet) > 0.25
    assert voice_level(silence) < 0.08
    assert voice_level(loud) > 0.8
    assert voice_level([]) == 0.0


def test_whatsapp_address_book(tmp_path, monkeypatch):
    db = tmp_path / "wa.db"
    con = sqlite3.connect(db)
    con.execute(
        "CREATE TABLE whatsmeow_contacts ("
        "our_jid TEXT, their_jid TEXT, first_name TEXT, full_name TEXT, "
        "push_name TEXT, business_name TEXT, redacted_phone TEXT)"
    )
    con.executemany(
        "INSERT INTO whatsmeow_contacts VALUES (?,?,?,?,?,?,?)",
        [
            ("self@s.whatsapp.net", "910000000009@s.whatsapp.net", "", "Mom", "Mom", "", ""),
            ("self@s.whatsapp.net", "910000000008@s.whatsapp.net", "", "Sam", "", "", ""),
            ("self@s.whatsapp.net", "910000000007@s.whatsapp.net", "", "Sam", "", "", ""),
        ],
    )
    con.commit()
    con.close()
    monkeypatch.setattr("tools.wa_link.session_path", lambda: str(db))
    monkeypatch.setattr(whatsapp, "load_contacts", lambda: {"me": "910000000001"})
    book = whatsapp.address_book()
    found = whatsapp.resolve_contact("mom", book)
    assert found is not None
    assert found[0] == "Mom"
    assert found[1] == "910000000009"
    assert whatsapp.resolve_contact("me", book)[0] == "me"
    assert whatsapp.resolve_contact("sam", book) is None
    assert whatsapp.resolve_contact("zzzznotaperson", book) is None
