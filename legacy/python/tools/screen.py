"""Screen perception tool for Kuro.

Captures active display using mss (with ImageGrab fallback), analyzes it using Ollama
vision model (qwen3-vl:8b), or falls back to OCR via pytesseract.
"""

import io
import os
import base64
import logging
from typing import Any
from PIL import Image
import config

logger = logging.getLogger(__name__)

NAME = "read_screen"
DESCRIPTION = (
    "Capture what is currently displayed on the user's screen and inspect or summarize it. "
    "Use when the user asks 'what is on my screen', 'read my screen', 'what does this error say', "
    "'what am I looking at', or asks about content currently visible on display."
)

PARAMETERS = {
    "type": "object",
    "properties": {
        "question": {
            "type": "string",
            "description": "The specific question to answer about the screen content (e.g. 'summarize the active window', 'what error is shown?')."
        }
    }
}

def capture_screen_image() -> tuple[Image.Image, str]:
    """
    Captures screen using mss, with ImageGrab fallback.
    Returns (PIL.Image, base64_jpeg_string).
    """
    img = None

    # Try mss first
    try:
        import mss
        with mss.mss() as sct:
            monitor = sct.monitors[1] if len(sct.monitors) > 1 else sct.monitors[0]
            sct_img = sct.grab(monitor)
            img = Image.frombytes("RGB", sct_img.size, sct_img.bgra, "raw", "BGRX")
    except Exception as mss_err:
        logger.debug(f"mss capture failed ({mss_err}), trying PIL.ImageGrab fallback...")
        try:
            from PIL import ImageGrab
            img = ImageGrab.grab(all_screens=True).convert("RGB")
        except Exception as grab_err:
            raise RuntimeError(f"Could not capture screen: {grab_err}")

    if img is None:
        raise RuntimeError("Screen capture returned empty image.")

    # Downscale if larger than 1600px to save VRAM and latency
    max_dim = 1600
    if max(img.size) > max_dim:
        img.thumbnail((max_dim, max_dim), Image.Resampling.LANCZOS)

    buffer = io.BytesIO()
    img.save(buffer, format="JPEG", quality=85, optimize=True)
    raw_bytes = buffer.getvalue()
    b64_str = base64.b64encode(raw_bytes).decode("utf-8")
    return img, b64_str

def run(arguments: dict[str, Any]) -> str:
    question = arguments.get("question", "").strip()
    if not question:
        question = "What is currently shown on the screen? Summarize key elements and active text."

    logger.info(f"Screen capture triggered with query: {question}")

    try:
        pil_img, b64_img = capture_screen_image()
    except Exception as e:
        logger.warning(f"Screen capture error: {e}")
        return "I could not capture your display right now. The screen may be locked or unfocused."

    # 1. Attempt Vision Model via Ollama
    vision_model = getattr(config, "OLLAMA_VISION_MODEL", "qwen3-vl:8b")
    if vision_model:
        try:
            import ollama
            client = ollama.Client()
            response = client.generate(
                model=vision_model,
                prompt=f"You are Kuro, a witty personal assistant looking at the user's screen. Answer concisely: {question}",
                images=[b64_img]
            )
            summary = response.get("response", "").strip()
            if summary:
                return f"Screen observation: {summary}"
        except Exception as e:
            logger.warning(f"Ollama vision model '{vision_model}' failed or not available: {e}. Trying OCR fallback...")

    # 2. Fallback to OCR via pytesseract
    try:
        import pytesseract
        text = pytesseract.image_to_string(pil_img).strip()
        if text:
            extracted = text[:500].replace("\n\n", "\n")
            return f"Screen OCR text detected:\n{extracted}"
    except Exception as e:
        logger.warning(f"OCR fallback failed: {e}")

    return "Screenshot captured, but neither vision model nor OCR could parse the visual content."
