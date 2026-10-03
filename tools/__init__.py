"""Auto-discovering tool registry for Kuro.

Any Python file added to this directory with:
- NAME: str
- DESCRIPTION: str
- PARAMETERS: dict (JSON Schema)
- run(args: dict) -> str or dict

is automatically discovered and made available to the LLM.
"""

import os
import importlib
import logging
from typing import Callable, Any

logger = logging.getLogger(__name__)

class ToolRegistry:
    def __init__(self):
        self._tools: dict[str, dict[str, Any]] = {}
        self.reload()

    def reload(self):
        """Scans the tools directory and loads all valid tool definitions."""
        self._tools.clear()
        tools_dir = os.path.dirname(__file__)

        for filename in os.listdir(tools_dir):
            if filename.endswith(".py") and not filename.startswith("__"):
                module_name = filename[:-3]
                try:
                    module = importlib.import_module(f"tools.{module_name}")
                    name = getattr(module, "NAME", module_name)
                    description = getattr(module, "DESCRIPTION", "")
                    parameters = getattr(module, "PARAMETERS", {"type": "object", "properties": {}})
                    run_func = getattr(module, "run", None)

                    if callable(run_func):
                        self._tools[name] = {
                            "name": name,
                            "description": description,
                            "parameters": parameters,
                            "run": run_func
                        }
                        logger.info(f"Loaded tool: {name}")
                except Exception as e:
                    logger.error(f"Failed to load tool module '{module_name}': {e}")

    def get_definitions(self) -> list[dict]:
        """Returns tool specs formatted for Ollama tool calling."""
        specs = []
        for tool in self._tools.values():
            specs.append({
                "type": "function",
                "function": {
                    "name": tool["name"],
                    "description": tool["description"],
                    "parameters": tool["parameters"]
                }
            })
        return specs

    def execute(self, name: str, arguments: dict) -> str:
        """Executes a tool by name with arguments. Never raises exceptions."""
        if name not in self._tools:
            return f"Error: Tool '{name}' is not recognized."

        tool = self._tools[name]
        try:
            result = tool["run"](arguments or {})
            if isinstance(result, str):
                return result
            return str(result)
        except Exception as e:
            logger.error(f"Error executing tool '{name}': {e}", exc_info=True)
            return f"Tool execution failed: {e}"

# Global registry singleton
registry = ToolRegistry()

def get_tool_definitions() -> list[dict]:
    return registry.get_definitions()

def execute_tool(name: str, arguments: dict) -> str:
    return registry.execute(name, arguments)
