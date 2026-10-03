"""Windows Power & Battery detection for Kuro.

Uses Windows kernel32 GetSystemPowerStatus to check whether the laptop is
connected to AC power (charging) and query the battery percentage.
Requires zero external packages.
"""

import ctypes
from ctypes import wintypes
import logging

logger = logging.getLogger(__name__)

class SYSTEM_POWER_STATUS(ctypes.Structure):
    _fields_ = [
        ("ACLineStatus", wintypes.BYTE),       # 0: Offline (battery), 1: Online (plugged in), 255: Unknown
        ("BatteryFlag", wintypes.BYTE),        # 1: High, 2: Low, 4: Critical, 8: Charging, 128: No battery
        ("BatteryLifePercent", wintypes.BYTE), # 0-100, 255: Unknown
        ("SystemStatusFlag", wintypes.BYTE),   # Reserved
        ("BatteryLifeTime", wintypes.DWORD),   # Remaining seconds, -1 if unknown
        ("BatteryFullLifeTime", wintypes.DWORD)# Full battery lifetime, -1 if unknown
    ]

def get_power_status() -> dict:
    """
    Queries current AC power and battery status.
    Returns:
        dict with keys: 'is_charging' (bool), 'battery_percent' (int or None), 'is_battery_present' (bool)
    """
    sps = SYSTEM_POWER_STATUS()
    if not ctypes.windll.kernel32.GetSystemPowerStatus(ctypes.byref(sps)):
        logger.warning("Failed to query GetSystemPowerStatus. Assuming AC power is connected.")
        return {"is_charging": True, "battery_percent": None, "is_battery_present": False}

    is_charging = (sps.ACLineStatus == 1)
    is_battery_present = not bool(sps.BatteryFlag & 128)
    battery_percent = int(sps.BatteryLifePercent) if sps.BatteryLifePercent != 255 else None

    # If it's a desktop without battery, ACLineStatus is usually 1 (Online)
    return {
        "is_charging": is_charging or not is_battery_present,
        "battery_percent": battery_percent,
        "is_battery_present": is_battery_present,
        "ac_line_status": sps.ACLineStatus
    }

def is_charging() -> bool:
    """Returns True if the device is plugged in or has no battery (desktop)."""
    return get_power_status()["is_charging"]

def get_battery_percent() -> int | None:
    """Returns battery percentage (0-100) or None if unavailable."""
    return get_power_status()["battery_percent"]
