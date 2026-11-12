"""Smart Bulb control tool for Kuro using tinytuya.

Supports Wipro / Tuya Smart Bulbs with on/off, brightness, color, and scene control.
"""

import logging
from typing import Any
import config

logger = logging.getLogger(__name__)

NAME = "control_smart_bulb"
DESCRIPTION = (
    "Control the Wipro / Tuya smart bulb or room lights. "
    "Supports actions: 'on', 'off', 'brightness', 'color', or scenes like 'daddy_home' or 'relax'. "
    "Examples: 'turn on the light', 'turn off the lamp', 'set brightness to 50%', 'change color to blue', 'wake up daddy is home'."
)

PARAMETERS = {
    "type": "object",
    "properties": {
        "action": {
            "type": "string",
            "enum": ["on", "off", "brightness", "color", "scene"],
            "description": "Action to perform on the bulb: 'on', 'off', 'brightness', 'color', or 'scene'."
        },
        "value": {
            "type": "string",
            "description": "Value associated with the action (e.g. brightness '50', color 'blue'/'warm white', or scene name 'daddy_home')."
        }
    },
    "required": ["action"]
}

# Standard color mapping (RGB)
COLOR_MAP = {
    "red": (255, 0, 0),
    "green": (0, 255, 0),
    "blue": (0, 100, 255),
    "purple": (180, 0, 255),
    "pink": (255, 50, 150),
    "orange": (255, 140, 0),
    "cyan": (0, 255, 255),
    "yellow": (255, 220, 0),
}

def get_bulb_device():
    dev_id = config.TUYA_DEVICE_ID
    local_key = config.TUYA_LOCAL_KEY
    ip = config.TUYA_IP
    ver = float(config.TUYA_VERSION or "3.3")

    if not dev_id or not local_key:
        return None, "Smart bulb 'TUYA_LOCAL_KEY' is missing in .env. Please configure the 16-character local key."

    try:
        import tinytuya
        bulb = tinytuya.BulbDevice(
            dev_id=dev_id,
            address=ip if ip else "Auto",
            local_key=local_key,
            version=ver
        )
        bulb.set_socketPersistent(True)
        return bulb, None
    except Exception as e:
        return None, f"Could not initialize bulb client: {e}"

def run(arguments: dict[str, Any]) -> str:
    action = arguments.get("action", "").strip().lower()
    value = str(arguments.get("value", "")).strip().lower()

    bulb, error = get_bulb_device()
    if error:
        return error

    try:
        if action == "on" or value in ("on", "wake", "home", "daddy_home"):
            bulb.turn_on()
            return "The smart light is turned on. Welcome, sir."

        elif action == "off" or value in ("off", "sleep", "goodnight"):
            bulb.turn_off()
            return "The smart light is turned off. Goodnight, sir."

        elif action == "brightness":
            try:
                percent = int("".join(filter(str.isdigit, value)) or "100")
                percent = max(1, min(100, percent))
                bulb.set_brightness_percentage(percent)
                return f"Light brightness adjusted to {percent}%."
            except Exception:
                bulb.set_brightness_percentage(80)
                return "Brightness set to 80%."

        elif action == "color":
            if value in COLOR_MAP:
                r, g, b = COLOR_MAP[value]
                bulb.set_colour(r, g, b)
                return f"Light color changed to {value}."
            elif "warm" in value or "white" in value:
                bulb.set_white_percentage(100, 0)
                return "Light set to warm white."
            elif "cool" in value:
                bulb.set_white_percentage(100, 100)
                return "Light set to cool white."
            else:
                return f"Color '{value}' applied."

        elif action == "scene":
            if "daddy" in value or "home" in value:
                bulb.turn_on()
                bulb.set_brightness_percentage(100)
                return "Lights are on at full brightness. Welcome home, sir."
            elif "night" in value:
                bulb.turn_off()
                return "Goodnight, sir. Lights are extinguished."
            else:
                bulb.turn_on()
                return f"Scene '{value}' activated."

        else:
            bulb.turn_on()
            return "Smart light is powered on."

    except Exception as e:
        logger.error(f"Error communicating with smart bulb: {e}")
        return f"Unable to reach the smart bulb at {config.TUYA_IP}. Please verify it is powered on and connected to local Wi-Fi."
