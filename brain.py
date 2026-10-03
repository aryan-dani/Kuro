"""Ollama brain & tool calling — lazy-loaded, never required at boot."""

from __future__ import annotations

import json
import logging
from typing import Any, Callable

import config
import tools

logger = logging.getLogger(__name__)

SYSTEM_PROMPT = """You are Kuro, a witty, dry-humored, and exceptionally capable personal AI assistant running locally on the user's Windows machine.
Personality guidelines:
1. Tone: Like Jarvis — intelligent, calm, dry-witted, concise, and professional. Never sound like a robotic customer support agent.
2. Brevity: Your replies will be spoken out loud. Keep answers to 1-2 short sentences. No lists or markdown.
3. Confirmation: Always clearly confirm physical or external actions taken (e.g. "Lights and charger are powered on.") rather than remaining silent or vague.
4. Tools: Use tools for news, screen reading, or smart home. For coming home / "daddy's home" / wake-up home cues, use control_smart_plug with scene daddy_home.
5. Speed: Prefer the smallest correct action. Do not over-explain.
"""

_brain: "Brain | None" = None


class Brain:
    def __init__(self, model_name: str | None = None):
        import ollama  # lazy — keeps boot free of LLM client init

        self.model = model_name or config.OLLAMA_MODEL
        self.client = ollama.Client()
        logger.info("Ollama brain client ready (model=%s)", self.model)

    def process(self, user_text: str, on_tool_start: Callable[[str], None] | None = None) -> str:
        if not user_text or not user_text.strip():
            return "I am listening, sir, but heard nothing."

        messages = [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_text},
        ]

        tool_specs = tools.get_tool_definitions()

        try:
            response = self.client.chat(
                model=self.model,
                messages=messages,
                tools=tool_specs,
                options={"temperature": 0.4, "num_predict": 128},
            )
        except Exception as e:
            logger.error("Failed to communicate with Ollama: %s", e)
            return "I cannot reach my neural engine. Please ensure Ollama is running."

        message = response.get("message", {})
        tool_calls = message.get("tool_calls", [])

        if not tool_calls:
            content = message.get("content", "").strip()
            return content or "Understood, sir."

        messages.append(message)

        for tool_call in tool_calls:
            func = tool_call.get("function", {})
            name = func.get("name")
            arguments = func.get("arguments", {})

            if isinstance(arguments, str):
                try:
                    arguments = json.loads(arguments)
                except Exception:
                    arguments = {}

            print(f"[Kuro Brain] Executing tool '{name}' with arguments: {arguments}")
            logger.info("Brain tool call: %s %s", name, arguments)
            if on_tool_start and name:
                try:
                    on_tool_start(name)
                except Exception:
                    pass

            result_text = tools.execute_tool(name, arguments or {})
            messages.append({"role": "tool", "content": result_text})

            if result_text.startswith("Success:"):
                return result_text.replace("Success:", "", 1).strip()
            if result_text.lower().startswith("latest headlines"):
                lines = [ln.strip("- ").strip() for ln in result_text.splitlines()[1:4] if ln.strip()]
                titles = []
                for ln in lines:
                    if "] " in ln:
                        ln = ln.split("] ", 1)[-1]
                    titles.append(ln.split(":", 1)[0].strip())
                if titles:
                    return "Headlines: " + ". ".join(titles[:3]) + "."

        try:
            final_response = self.client.chat(
                model=self.model,
                messages=messages,
                options={"temperature": 0.3, "num_predict": 80},
            )
            final_content = final_response.get("message", {}).get("content", "").strip()
            return final_content or "Action completed, sir."
        except Exception as e:
            logger.error("Error in final Ollama response pass: %s", e)
            return "I executed the command, but had trouble articulating the outcome."


def think(user_text: str, on_tool_start: Callable[[str], None] | None = None) -> str:
    """Lazy LLM path — only constructs Brain on first freeform ask."""
    mode = config.KURO_LLM_MODE
    if mode == "off":
        return "No command matched."

    global _brain
    if _brain is None:
        try:
            _brain = Brain()
        except Exception as e:
            logger.exception("Brain init failed")
            return "I cannot reach my neural engine right now."

    return _brain.process(user_text, on_tool_start)
