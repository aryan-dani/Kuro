"""Pause Spotify and any other playback while Kuro is listening, then resume it."""

from __future__ import annotations

import logging
import subprocess
import threading

import music

logger = logging.getLogger(__name__)

_lock = threading.Lock()
_ids: list[str] | None = None
_local = False

_PAUSE_SCRIPT = r"""
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
})[0]
function Await($WinRtTask, $ResultType) {
  $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
  $netTask = $asTask.Invoke($null, @($WinRtTask))
  $netTask.Wait(-1) | Out-Null
  $netTask.Result
}
$null = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager,Windows.Media.Control,ContentType=WindowsRuntime]
$mgr = Await ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager]::RequestAsync()) ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager])
foreach ($s in @($mgr.GetSessions())) {
  try {
    $info = $s.GetPlaybackInfo()
    if ([string]$info.PlaybackStatus -ne 'Playing') { continue }
    $id = [string]$s.SourceAppUserModelId
    if (-not $id) { continue }
    $ok = Await ($s.TryPauseAsync()) ([bool])
    if ($ok) { Write-Output ('PAUSED ' + $id) }
  } catch {}
}
"""


def _powershell(script: str) -> str:
    done = subprocess.run(
        ["powershell", "-NoProfile", "-Sta", "-Command", script],
        capture_output=True,
        text=True,
        timeout=4,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    )
    return done.stdout or ""


def _pause_system() -> list[str]:
    ids: list[str] = []
    for line in _powershell(_PAUSE_SCRIPT).splitlines():
        if line.startswith("PAUSED "):
            app_id = line[7:].strip()
            if app_id:
                ids.append(app_id)
    return ids


def _resume_system(ids: list[str]) -> None:
    wanted = ",".join("'" + app_id.replace("'", "''") + "'" for app_id in ids)
    script = _PAUSE_SCRIPT.replace(
        "if ([string]$info.PlaybackStatus -ne 'Playing') { continue }",
        "if ([string]$info.PlaybackStatus -ne 'Paused') { continue }",
    ).replace(
        "$ok = Await ($s.TryPauseAsync()) ([bool])",
        f"if (@({wanted}) -notcontains $id) {{ continue }}\n    $ok = Await ($s.TryPlayAsync()) ([bool])",
    )
    _powershell(script)


def _pause_local() -> bool:
    try:
        if not music.output_active():
            return False
        music.pause_song()
        return True
    except Exception:
        logger.info("Local music pause failed")
        return False


def _resume_local() -> None:
    try:
        if music.is_paused():
            music.resume_song()
    except Exception:
        logger.info("Local music resume failed")


def hold() -> None:
    """Pause whatever is playing. A second call keeps the same list."""
    global _ids, _local
    with _lock:
        if _ids is not None:
            return
        local = False
        ids: list[str] = []
        try:
            local = _pause_local()
            ids = _pause_system()
        except Exception:
            logger.info("Playback pause failed")
        _local = local
        _ids = ids
        if local or ids:
            logger.info("Paused playback for listening (%s)", len(ids) + int(local))


def release() -> None:
    """Resume only what hold() paused."""
    global _ids, _local
    with _lock:
        ids = _ids or []
        local = _local
        _ids = None
        _local = False
    if local:
        _resume_local()
    if ids:
        try:
            _resume_system(ids)
            logger.info("Resumed playback")
        except Exception:
            logger.info("Playback resume failed")


def forget() -> None:
    """Drop the resume list. The user asked for the music to stay stopped."""
    global _ids, _local
    with _lock:
        _ids = None
        _local = False
