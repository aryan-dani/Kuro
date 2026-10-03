import os
from dotenv import load_dotenv

# Override so .env wins over stale shell env (e.g. old ONLY_WHEN_CHARGING=true).
load_dotenv(override=True)

KURO_HOTKEY = os.getenv("KURO_HOTKEY", "ctrl+alt+space")

# Power Policy: Only active when charging / on AC power
ONLY_WHEN_CHARGING = os.getenv("ONLY_WHEN_CHARGING", "false").lower() in ("true", "1", "yes")

# Ollama models
OLLAMA_MODEL = os.getenv("OLLAMA_MODEL", "qwen2.5:7b-instruct")
OLLAMA_VISION_MODEL = os.getenv("OLLAMA_VISION_MODEL", "qwen3-vl:8b")

# Phrase-first agent: off = never call LLM; fallback = LLM only for unknown speech; always = LLM anytime
_KURO_LLM_RAW = os.getenv("KURO_LLM_MODE", "fallback").strip().lower()
KURO_LLM_MODE = _KURO_LLM_RAW if _KURO_LLM_RAW in ("off", "fallback", "always") else "fallback"

# Coqui XTTS-v2 Settings
XTTS_SPEAKER_WAV = os.getenv("XTTS_SPEAKER_WAV", "")
XTTS_LANGUAGE = os.getenv("XTTS_LANGUAGE", "en")

# Tuya smart plug settings
TUYA_DEVICE_ID = os.getenv("TUYA_DEVICE_ID", "")
TUYA_LOCAL_KEY = os.getenv("TUYA_LOCAL_KEY", "")
TUYA_IP = os.getenv("TUYA_IP", "")
TUYA_VERSION = os.getenv("TUYA_VERSION", "3.3")
