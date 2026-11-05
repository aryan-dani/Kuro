"""Registry matching, phrase routing, and mocked n8n HTTP."""

from __future__ import annotations

import io
import json
import urllib.error

import pytest

import config
import phrases
from tools import n8n

FIXTURE = [
    {
        "id": "morning_brief",
        "webhook": "morning-brief",
        "description": "Today",
        "aliases": ["morning brief", "my morning", "start my day"],
        "mode": "sync",
        "takes_input": False,
        "spoken_name": "morning brief",
    },
    {
        "id": "log_idea",
        "webhook": "log-idea",
        "description": "Notes",
        "aliases": ["log idea", "save idea", "note this"],
        "mode": "async",
        "takes_input": True,
        "spoken_name": "log idea",
    },
    {
        "id": "hackathon_prep",
        "webhook": "hackathon-prep",
        "description": "Prep",
        "aliases": ["hackathon prep"],
        "mode": "sync",
        "takes_input": False,
        "spoken_name": "hackathon prep",
    },
]


@pytest.fixture
def workflows(monkeypatch):
    monkeypatch.setattr(n8n, "load_workflows", lambda: FIXTURE)
    return FIXTURE


def test_alias_exact_and_fuzzy(workflows):
    assert n8n.resolve_workflow("morning brief", workflows)["id"] == "morning_brief"
    assert n8n.resolve_workflow("morning_brief", workflows)["id"] == "morning_brief"
    assert n8n.resolve_workflow("morning breif", workflows)["id"] == "morning_brief"
    assert n8n.resolve_workflow("banana radio", workflows) is None
    assert n8n.resolve_workflow("zzzzzzzzzzzz", workflows) is None


def test_input_capture(workflows):
    wf, extra = n8n.match_phrase("log idea build the nfc reader", workflows)
    assert wf["id"] == "log_idea"
    assert extra == "build the nfc reader"
    wf, extra = n8n.match_phrase("note this ship the reader", workflows)
    assert extra == "ship the reader"


def test_phrase_phrasings(workflows):
    samples = [
        "run morning brief",
        "please run the morning brief",
        "hey kuro start my morning",
        "can you launch hackathon prep",
        "trigger morning brief",
        "start my day",
        "morning brief",
        "my morning",
        "please log idea buy milk",
        "save idea the drawer sensor",
        "kindly run hackathon prep",
        "n8n is not this",
    ]
    hits = [n8n.match_phrase(text, workflows) for text in samples]
    assert hits[0][0]["id"] == "morning_brief"
    assert hits[1][0]["id"] == "morning_brief"
    assert hits[2][0]["id"] == "morning_brief"
    assert hits[3][0]["id"] == "hackathon_prep"
    assert hits[4][0]["id"] == "morning_brief"
    assert hits[5][0]["id"] == "morning_brief" and hits[5][1] == ""
    assert hits[6][0]["id"] == "morning_brief"
    assert hits[7][0]["id"] == "morning_brief"
    assert hits[8][0]["id"] == "log_idea" and hits[8][1] == "buy milk"
    assert hits[9][0]["id"] == "log_idea" and "drawer" in hits[9][1]
    assert hits[10][0]["id"] == "hackathon_prep"
    assert hits[11] is None


def test_phrases_matches_routes(workflows):
    assert phrases.matches("please run the morning brief")
    assert phrases.matches("log idea build the nfc reader")
    assert not phrases.matches("run banana radio")
    assert phrases.matches("desk off")
    assert phrases.matches("what's the news today")


class _Resp:
    def __init__(self, body: str, status: int = 200):
        self.status = status
        self._body = body.encode()

    def read(self):
        return self._body

    def __enter__(self):
        return self

    def __exit__(self, *args):
        return False


@pytest.fixture
def configured(monkeypatch):
    monkeypatch.setattr(config, "KURO_N8N_BASE_URL", "http://100.78.60.50:5678/webhook")
    monkeypatch.setattr(config, "KURO_N8N_TOKEN", "test-token")


def _wf(mode="sync"):
    return {
        "id": "hello_kuro",
        "webhook": "hello-kuro",
        "mode": mode,
        "spoken_name": "hello kuro",
    }


def test_http_cases(configured):
    wf = _wf("sync")
    assert n8n.dispatch(wf, "", lambda req, timeout: _Resp('{"reply":"Hello from n8n"}')).endswith(
        "Hello from n8n"
    )
    assert n8n.dispatch(wf, "", lambda req, timeout: _Resp("{}")) == "Success: Done, sir."

    def denied(req, timeout):
        raise urllib.error.HTTPError(req.full_url, 401, "no", hdrs=None, fp=io.BytesIO(b""))

    def missing(req, timeout):
        raise urllib.error.HTTPError(req.full_url, 404, "no", hdrs=None, fp=io.BytesIO(b""))

    def timed_out(req, timeout):
        raise TimeoutError("slow")

    def refused(req, timeout):
        raise urllib.error.URLError("connection refused")

    assert n8n.dispatch(wf, "", denied) == "n8n rejected my token."
    assert n8n.dispatch(wf, "", missing) == "That workflow is not active in n8n."
    assert n8n.dispatch(wf, "", timed_out) == "n8n timed out."
    assert n8n.dispatch(wf, "", refused) == "I can't reach n8n."
    assert n8n.dispatch(wf, "", lambda req, timeout: _Resp("not json")) == (
        "n8n returned something I could not read."
    )
    started = n8n.dispatch(_wf("async"), "note", lambda req, timeout: _Resp("", 200))
    assert started == "Success: Started hello kuro."


def test_unconfigured(monkeypatch):
    monkeypatch.setattr(config, "KURO_N8N_BASE_URL", "")
    monkeypatch.setattr(config, "KURO_N8N_TOKEN", "")
    assert n8n.dispatch(_wf(), "", lambda req, timeout: (_ for _ in ()).throw(AssertionError("called"))) == (
        "n8n is not configured."
    )


def test_registry_parse(tmp_path, monkeypatch):
    path = tmp_path / "n8n_workflows.json"
    path.write_text(
        json.dumps(
            {
                "workflows": {
                    "hello_kuro": {
                        "webhook": "hello-kuro",
                        "description": "Hi",
                        "aliases": ["Hello Workflow!"],
                        "mode": "nope",
                        "takes_input": False,
                    }
                }
            }
        ),
        encoding="utf-8",
    )
    monkeypatch.setattr(n8n, "registry_path", lambda: str(path))
    found = n8n.load_workflows()
    assert found[0]["aliases"] == ["hello workflow"]
    assert found[0]["mode"] == "sync"
    monkeypatch.setattr(n8n, "registry_path", lambda: str(tmp_path / "missing.json"))
    assert n8n.load_workflows() == []
