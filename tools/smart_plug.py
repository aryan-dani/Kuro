"""Smart plug control for Kuro via tinytuya (local LAN).

Discovers the device IP automatically when the configured address is stale
(DHCP churn) and caches the working endpoint for fast follow-up commands.
"""

from __future__ import annotations

import json
import logging
import os
import time
from pathlib import Path
from typing import Any

import config

logger = logging.getLogger(__name__)

NAME = "control_smart_plug"
DESCRIPTION = (
    "Control the desk extension board via smart plug (lights + monitor AC). "
    "Router/laptop charger must stay on always-on wall power — never on this board. "
    "Use for lights/monitor/desk on/off, good night, daddy_home."
)

PARAMETERS = {
    "type": "object",
    "properties": {
        "scene": {
            "type": "string",
            "enum": ["daddy_home", "good_night", "power_on", "power_off"],
            "description": (
                "Scene: 'daddy_home' / 'power_on' turn the plug ON; "
                "'good_night' / 'power_off' turn it OFF."
            ),
        }
    },
    "required": ["scene"],
}

SCENES = {
    "daddy_home": {"action": "on", "description": "Desk power is on. Welcome home, sir."},
    "power_on": {"action": "on", "description": "Desk on — lights and monitor powered."},
    "good_night": {"action": "off", "description": "Desk off. Goodnight, sir."},
    "power_off": {"action": "off", "description": "Desk off — lights and monitor unpowered."},
}

_CACHE = Path(os.environ.get("LOCALAPPDATA", ".")) / "Kuro" / "tuya_endpoint.json"
_VERSIONS = (3.5, 3.4, 3.3, 3.1)
_live = None


def _load_cache() -> dict:
    try:
        if _CACHE.is_file():
            return json.loads(_CACHE.read_text(encoding="utf-8"))
    except Exception:
        pass
    return {}


def _save_cache(ip: str, version: float) -> None:
    try:
        _CACHE.parent.mkdir(parents=True, exist_ok=True)
        _CACHE.write_text(
            json.dumps({"ip": ip, "version": version, "updated": time.time()}, indent=2),
            encoding="utf-8",
        )
    except Exception:
        logger.debug("endpoint cache write failed", exc_info=True)


def _candidate_ips() -> list[str]:
    ips: list[str] = []
    cached = _load_cache().get("ip")
    configured = (config.TUYA_IP or "").strip()
    for ip in (cached, configured, "Auto"):
        if ip and ip not in ips:
            ips.append(ip)
    return ips


def _candidate_versions() -> list[float]:
    vers: list[float] = []
    cached = _load_cache().get("version")
    configured = float(config.TUYA_VERSION) if config.TUYA_VERSION else 3.5
    for v in (cached, configured, *_VERSIONS):
        try:
            fv = float(v)
        except (TypeError, ValueError):
            continue
        if fv not in vers:
            vers.append(fv)
    return vers


def _make_device(ip: str, version: float):
    import tinytuya

    plug = tinytuya.OutletDevice(
        dev_id=config.TUYA_DEVICE_ID,
        address=ip,
        local_key=config.TUYA_LOCAL_KEY,
        version=version,
    )
    plug.set_socketTimeout(4)
    plug.set_socketPersistent(False)
    return plug


def _status_ok(data: Any) -> bool:
    return isinstance(data, dict) and "dps" in data and not data.get("Error")


def _connect() -> tuple[Any, str]:
    """Return (device, human_error). device is None on failure."""
    global _live
    if _live is not None:
        try:
            if _status_ok(_live.status()):
                return _live, ""
        except Exception:
            _live = None
    last_err = "unreachable"
    for ip in _candidate_ips():
        for ver in _candidate_versions():
            try:
                plug = _make_device(ip, ver)
                st = plug.status()
                if _status_ok(st):
                    resolved = getattr(plug, "address", ip) or ip
                    logger.info("Tuya plug online at %s (v%s)", resolved, ver)
                    try:
                        plug.set_socketPersistent(True)
                    except Exception:
                        pass
                    _live = plug
                    if resolved and resolved != "Auto":
                        _save_cache(str(resolved), float(ver))
                    return plug, ""
                last_err = str((st or {}).get("Error") or (st or {}).get("Err") or st)
                logger.debug("Tuya %s v%s -> %s", ip, ver, last_err)
            except Exception as e:
                last_err = str(e)
                logger.debug("Tuya %s v%s exc: %s", ip, ver, e)
    return None, last_err


def probe_status() -> str:
    """Read the plug. Does not change the relay."""
    import timing

    started = timing.tick()
    try:
        plug, _err = _connect()
        if plug is None:
            return "unreachable"
        plug.status()
        return "ok"
    except Exception:
        return "unreachable"
    finally:
        timing.once("plug", "status", timing.since(started))


def run(arguments: dict[str, Any]) -> str:
    scene = (arguments.get("scene") or "").strip().lower()
    if not scene or scene not in SCENES:
        if any(k in scene for k in ("home", "daddy", "wake", "on", "light")) and "off" not in scene:
            scene = "power_on"
        elif any(k in scene for k in ("night", "sleep", "off")):
            scene = "power_off"
        else:
            return f"Unknown scene '{scene}'. Available: {', '.join(SCENES)}."

    scene_info = SCENES[scene]
    action = scene_info["action"]

    if not config.TUYA_DEVICE_ID or not config.TUYA_LOCAL_KEY:
        return "Smart plug credentials are missing in .env."

    plug, err = _connect()
    if plug is None:
        logger.warning("Smart plug unreachable: %s", err)
        return (
            "I can't reach the smart plug on the Wi-Fi right now. "
            "Check that it is powered and on the same network."
        )

    try:
        data = plug.turn_on() if action == "on" else plug.turn_off()
        if isinstance(data, dict) and data.get("Error"):
            # One retry with fresh Auto discovery
            logger.warning("Tuya action error: %s — rediscovering", data)
            try:
                _CACHE.unlink(missing_ok=True)
            except Exception:
                pass
            plug, err = _connect()
            if plug is None:
                return (
                    "I can't reach the smart plug on the Wi-Fi right now. "
                    "Check that it is powered and on the same network."
                )
            data = plug.turn_on() if action == "on" else plug.turn_off()
            if isinstance(data, dict) and data.get("Error"):
                logger.warning("Tuya retry failed: %s", data)
                return f"Smart plug error: {data.get('Err', 'offline')}."

        return f"Success: {scene_info['description']}"
    except Exception as e:
        logger.error("Smart plug command failed: %s", e)
        return "Unable to reach the smart plug right now."
