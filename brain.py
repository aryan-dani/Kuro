"""Ollama brain & tool calling — lazy-loaded, never required at boot."""

from __future__ import annotations

import json
import logging
import time
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
6. WhatsApp: if they ask to send the clipboard, a link, or a message and do not name another person, the contact is me. Do not invent a contact and do not ask who.
"""

_brain: "Brain | None" = None

# Tools that must stay on the laptop even if the hub also lists something similar.
_LOCAL_ONLY = {"read_screen", "check_inbox"}


def _merged_tool_specs() -> list[dict]:
    """Local tools plus hub schemas; hub wins for shared names except local-only."""
    import hub_client

    local = {t["function"]["name"]: t for t in tools.get_tool_definitions() if "function" in t}
    for spec in hub_client.list_tools():
        try:
            name = spec["function"]["name"]
        except Exception:
            continue
        if name in _LOCAL_ONLY:
            continue
        local[name] = spec
    return list(local.values())


def _hub_tool_names(specs: list[dict]) -> set[str]:
    import hub_client

    if not hub_client.configured():
        return set()
    names = set()
    for spec in hub_client.list_tools():
        try:
            name = spec["function"]["name"]
        except Exception:
            continue
        if name not in _LOCAL_ONLY:
            names.add(name)
    return names


class Brain:
    def __init__(self, model_name: str | None = None):
        import ollama  # lazy — keeps boot free of LLM client init

        self.model = model_name or config.OLLAMA_MODEL
        self.client = ollama.Client()
        self._messages: list[dict] = []
        self._updated = 0.0
        logger.info("Ollama brain client ready (model=%s)", self.model)

    def _thread(self, user_text: str) -> list[dict]:
        """Keep the conversation until it is cleared. Cap at 20 turns."""
        if not self._messages:
            self._messages = [{"role": "system", "content": SYSTEM_PROMPT}]
        self._messages.append({"role": "user", "content": user_text})
        self._updated = time.time()
        self._trim()
        return self._messages

    def _remember(self, reply: str) -> str:
        self._messages.append({"role": "assistant", "content": reply})
        self._updated = time.time()
        self._trim()
        return reply

    def _trim(self) -> None:
        if not self._messages:
            return
        system = self._messages[0]
        rest = self._messages[1:]
        if len(rest) > 40:
            rest = rest[-40:]
            if rest and rest[0].get("role") == "assistant":
                rest = rest[1:]
        self._messages = [system, *rest]

    def clear(self) -> None:
        self._messages = []
        self._updated = 0.0

    def process(self, user_text: str, on_tool_start: Callable[[str], None] | None = None) -> str:
        if not user_text or not user_text.strip():
            return "I am listening, sir, but heard nothing."

        messages = self._thread(user_text)

        tool_specs = _merged_tool_specs()
        hub_tool_names = _hub_tool_names(tool_specs)

        try:
            response = self.client.chat(
                model=self.model,
                messages=messages,
                tools=tool_specs,
                options={"temperature": 0.4, "num_predict": 128},
            )
        except Exception as e:
            logger.error("Failed to communicate with Ollama: %s", e)
            return "The neural engine is still starting. Try that again in a moment."

        message = response.get("message", {})
        tool_calls = message.get("tool_calls", [])

        if not tool_calls:
            content = message.get("content", "").strip() or "Understood, sir."
            return self._remember(content)

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

            if name in hub_tool_names:
                import hub_client

                result_text = hub_client.execute_tool(name, arguments or {})
            else:
                result_text = tools.execute_tool(name, arguments or {})
            messages.append({"role": "tool", "content": result_text})

            if result_text.startswith("Success:"):
                return self._remember(result_text.replace("Success:", "", 1).strip())
            if result_text.lower().startswith("latest headlines"):
                lines = [ln.strip("- ").strip() for ln in result_text.splitlines()[1:4] if ln.strip()]
                titles = []
                for ln in lines:
                    if "] " in ln:
                        ln = ln.split("] ", 1)[-1]
                    titles.append(ln.split(":", 1)[0].strip())
                if titles:
                    return self._remember("Headlines: " + ". ".join(titles[:3]) + ".")

        try:
            final_response = self.client.chat(
                model=self.model,
                messages=messages,
                options={"temperature": 0.3, "num_predict": 80},
            )
            final_content = final_response.get("message", {}).get("content", "").strip()
            return self._remember(final_content or "Action completed, sir.")
        except Exception as e:
            logger.error("Error in final Ollama response pass: %s", e)
            return self._remember("I executed the command, but had trouble articulating the outcome.")


def clear_thread() -> None:
    """Drop the model conversation. Does not log the text."""
    if _brain is not None:
        _brain.clear()


def think(user_text: str, on_tool_start: Callable[[str], None] | None = None) -> str:
    """Lazy LLM path — only constructs Brain on first freeform ask.

    Callers must ensure Ollama is up (see ollama_runtime.ensure_ready) before this
    when they want auto-start behavior. This function does not start the daemon.
    """
    mode = config.KURO_LLM_MODE
    if mode == "off":
        return "No command matched."

    global _brain
    if _brain is None:
        try:
            _brain = Brain()
        except Exception:
            logger.exception("Brain init failed")
            return "The neural engine failed to start."

    return _brain.process(user_text, on_tool_start)
