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

# STT backend: auto = CUDA on AC / OpenVINO NPU on battery (checked per transcription)
_KURO_STT_RAW = os.getenv("KURO_STT_BACKEND", "auto").strip().lower()
KURO_STT_BACKEND = _KURO_STT_RAW if _KURO_STT_RAW in ("auto", "cuda", "npu", "cpu") else "auto"

# TTS: auto = XTTS on AC / Piper on battery; or force xtts|piper|pyttsx3
_KURO_TTS_RAW = os.getenv("KURO_TTS_ENGINE", "auto").strip().lower()
KURO_TTS_ENGINE = (
    _KURO_TTS_RAW if _KURO_TTS_RAW in ("auto", "xtts", "piper", "pyttsx3") else "auto"
)

# Coqui XTTS-v2 Settings
XTTS_SPEAKER_WAV = os.getenv("XTTS_SPEAKER_WAV", "")
XTTS_LANGUAGE = os.getenv("XTTS_LANGUAGE", "en")

# Tuya smart plug settings
TUYA_DEVICE_ID = os.getenv("TUYA_DEVICE_ID", "")
TUYA_LOCAL_KEY = os.getenv("TUYA_LOCAL_KEY", "")
TUYA_IP = os.getenv("TUYA_IP", "")
TUYA_VERSION = os.getenv("TUYA_VERSION", "3.3")

# Cloud inbox (n8n) — optional; desk boot does not require these
N8N_INBOX_WEBHOOK_URL = os.getenv("N8N_INBOX_WEBHOOK_URL", "")
N8N_WEBHOOK_SECRET = os.getenv("N8N_WEBHOOK_SECRET", "")

# Kuro hub (Tailscale). Fill KURO_HUB_TOKEN to match /srv/kuro/.env on the server.
KURO_HUB_URL = os.getenv("KURO_HUB_URL", "http://100.78.60.50:18200").rstrip("/")
KURO_HUB_TOKEN = os.getenv("KURO_HUB_TOKEN", "")

