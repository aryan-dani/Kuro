"""Localhost engine API: auth, whitelist, and atomic file writes."""

from __future__ import annotations

import json

import pytest

import engine_api


@pytest.fixture
def token_file(monkeypatch, tmp_path):
    path = tmp_path / "api_token"
    path.write_text("test-token-value", encoding="utf-8")
    monkeypatch.setattr(engine_api, "token_path", lambda: path)
    engine_api.set_paused(False)
    return path


def test_missing_token_does_not_match_and_refuses(monkeypatch, tmp_path):
    missing = tmp_path / "nope"
    monkeypatch.setattr(engine_api, "token_path", lambda: missing)
    assert engine_api.load_token() == ""
    assert engine_api.token_matches("anything") is False
    assert engine_api.start_background() is False


def test_compare_digest_rejects_a_wrong_token(token_file):
    assert engine_api.token_matches("test-token-value") is True
    assert engine_api.token_matches("test-token-valuE") is False
    assert engine_api.token_matches("") is False


def test_config_whitelist_rejects_secrets_and_unknown_fields(token_file):
    import config

    previous = config.KURO_LLM_MODE
    try:
        engine_api.apply_config({"llm_mode": "off", "paused": True})
        assert engine_api.public_config()["llm_mode"] == "off"
        assert engine_api.is_paused() is True
        with pytest.raises(ValueError):
            engine_api.apply_config({"KURO_N8N_TOKEN": "nope"})
        with pytest.raises(ValueError):
            engine_api.apply_config({"llm_mode": "always-on"})
    finally:
        config.KURO_LLM_MODE = previous
        engine_api.set_paused(False)


def test_atomic_contacts_and_workflows(monkeypatch, tmp_path, token_file):
    contacts = tmp_path / "contacts.json"
    registry = tmp_path / "n8n_workflows.json"
    monkeypatch.setattr("tools.whatsapp.contacts_path", lambda: str(contacts))
    monkeypatch.setattr("tools.n8n.registry_path", lambda: str(registry))

    from fastapi.testclient import TestClient

    app = engine_api.build_app()
    client = TestClient(app)
    headers = {"X-Kuro-Token": "test-token-value"}

    bad = client.put("/contacts", headers=headers, json={"me": "12"})
    assert bad.status_code == 400
    assert not contacts.exists()

    good = client.put("/contacts", headers=headers, json={"me": "910000000001"})
    assert good.status_code == 200
    written = json.loads(contacts.read_text(encoding="utf-8"))
    assert written["me"] == "910000000001"
    listed = client.get("/contacts", headers=headers)
    assert listed.json()["contacts"]["me"].endswith("01")
    assert "910000000001" not in listed.text

    denied = client.get("/contacts")
    assert denied.status_code == 401

    wf = {"workflows": {"hello": {"webhook": "hello-kuro", "mode": "sync"}}}
    saved = client.put("/workflows", headers=headers, json=wf)
    assert saved.status_code == 200
    assert json.loads(registry.read_text(encoding="utf-8"))["workflows"]["hello"]["webhook"] == "hello-kuro"
    leaked = client.put(
        "/workflows",
        headers=headers,
        json={"workflows": {"bad": {"webhook": "http://example.test/hook"}}},
    )
    assert leaked.status_code == 400
    assert "example.test" not in registry.read_text(encoding="utf-8")


def test_say_uses_the_phrase_router(monkeypatch, token_file):
    calls = {}

    def _handle(text: str):
        calls["text"] = text
        return None

    monkeypatch.setattr("phrases.try_handle", _handle)
    from fastapi.testclient import TestClient

    client = TestClient(engine_api.build_app())
    response = client.post(
        "/say",
        headers={"X-Kuro-Token": "test-token-value"},
        json={"text": "pause the song"},
    )
    assert response.status_code == 200
    assert response.json()["matched"] is False
    assert calls["text"] == "pause the song"


def test_thread_clear_forgets_the_model_conversation(monkeypatch, token_file):
    import brain

    class _Brain:
        def __init__(self):
            self.cleared = False

        def clear(self):
            self.cleared = True

    stub = _Brain()
    monkeypatch.setattr(brain, "_brain", stub)
    from fastapi.testclient import TestClient

    client = TestClient(engine_api.build_app())
    denied = client.post("/thread/clear", json={})
    assert denied.status_code == 401
    assert stub.cleared is False
    ok = client.post("/thread/clear", headers={"X-Kuro-Token": "test-token-value"}, json={})
    assert ok.status_code == 200
    assert stub.cleared is True


def test_dismiss_hides_without_using_escape(token_file):
    called = {}
    engine_api.on_dismiss(lambda: called.setdefault("yes", True))
    from fastapi.testclient import TestClient

    client = TestClient(engine_api.build_app())
    denied = client.post("/dismiss", json={})
    assert denied.status_code == 401
    ok = client.post("/dismiss", headers={"X-Kuro-Token": "test-token-value"}, json={})
    assert ok.status_code == 200
    assert called.get("yes") is True


def test_event_frame_hides_text_unless_asked(monkeypatch):
    class _Hud:
        def snapshot(self):
            return {"state": "listening", "levels": [0.1] * 11}

        def transcript_pair(self):
            return "secret phrase", "reply phrase"

    monkeypatch.setattr("overlay.current", lambda: _Hud())
    quiet = engine_api.event_frame(include_text=False)
    assert "secret" not in json.dumps(quiet)
    shown = engine_api.event_frame(include_text=True)
    assert shown["you"] == "secret phrase"
