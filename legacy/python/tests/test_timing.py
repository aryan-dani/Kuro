"""Timing harness writes stage durations and nothing else."""

from __future__ import annotations

import csv

import timing


def test_timing_csv_has_stages_only(monkeypatch, tmp_path):
    monkeypatch.setattr(timing, "_ENABLED", True)
    path = tmp_path / "timing.csv"
    timing.set_path(path)
    try:
        command_id = timing.begin("music")
        timing.mark("hotkey_down")
        timing.mark("stt_done")
        timing.finish(command_id)
        timing.once("startup", "whisper_load", 12.5)
        rows = list(csv.DictReader(path.open(encoding="utf-8")))
    finally:
        timing.set_path(None)
        timing.finish(command_id)
    assert [row["stage"] for row in rows] == ["hotkey_down", "stt_done", "whisper_load"]
    blob = path.read_text(encoding="utf-8")
    assert "mom" not in blob.lower()
    assert "http" not in blob.lower()
    assert rows[0]["kind"] == "music"
    assert rows[2]["kind"] == "startup"
    assert float(rows[2]["ms"]) == 12.5


def test_timing_disabled_writes_nothing(monkeypatch, tmp_path):
    monkeypatch.setattr(timing, "_ENABLED", False)
    path = tmp_path / "timing.csv"
    timing.set_path(path)
    try:
        assert timing.begin("news") == ""
        timing.mark("stt_done")
        timing.once("plug", "status", 3.0)
    finally:
        timing.set_path(None)
    assert not path.exists()
