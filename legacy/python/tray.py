"""System tray icon and state indicator for Kuro using pystray and Pillow.

Provides background presence with visual feedback for:
- IDLE: Slate/Cyan orb
- LISTENING: Amber/Gold orb
- THINKING: Violet/Purple orb
- SPEAKING: Emerald/Green orb
"""

import threading
import logging
from enum import Enum
from typing import Callable, Any
from PIL import Image, ImageDraw
import pystray

import config
import power

logger = logging.getLogger(__name__)

class KuroState(Enum):
    IDLE = "idle"
    LISTENING = "listening"
    THINKING = "thinking"
    SPEAKING = "speaking"

# Color mappings for each state
STATE_COLORS = {
    KuroState.IDLE: {"fill": (40, 44, 52), "glow": (0, 200, 255), "center": (30, 144, 255)},
    KuroState.LISTENING: {"fill": (30, 30, 30), "glow": (255, 170, 0), "center": (255, 200, 50)},
    KuroState.THINKING: {"fill": (30, 20, 40), "glow": (180, 70, 255), "center": (210, 120, 255)},
    KuroState.SPEAKING: {"fill": (20, 40, 30), "glow": (46, 204, 113), "center": (80, 240, 150)},
}

def create_state_icon(state: KuroState, size: int = 64) -> Image.Image:
    """Generates a modern circular gradient orb icon for the given state."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    colors = STATE_COLORS.get(state, STATE_COLORS[KuroState.IDLE])

    # Outer border / subtle halo
    margin = 4
    draw.ellipse([margin, margin, size - margin, size - margin], fill=colors["fill"], outline=colors["glow"], width=3)
    
    # Inner luminous core
    inner_margin = 16
    draw.ellipse(
        [inner_margin, inner_margin, size - inner_margin, size - inner_margin],
        fill=colors["center"]
    )
    return img

class TrayManager:
    def __init__(self, on_trigger: Callable | None = None, on_quit: Callable | None = None):
        self._state = KuroState.IDLE
        self._on_trigger = on_trigger
        self._on_quit = on_quit
        self._icon: pystray.Icon | None = None
        self._thread: threading.Thread | None = None

    @property
    def state(self) -> KuroState:
        return self._state

    def set_state(self, new_state: KuroState):
        """Updates tray icon state and refreshes image."""
        self._state = new_state
        if self._icon:
            try:
                self._icon.icon = create_state_icon(new_state)
                self._icon.title = f"Kuro: {new_state.value.title()}"
            except Exception as e:
                logger.debug(f"Failed to update tray icon: {e}")

    def _get_status_text(self) -> str:
        p_status = power.get_power_status()
        batt = f"{p_status['battery_percent']}%" if p_status['battery_percent'] is not None else "N/A"
        charging_str = "Plugged In" if p_status['is_charging'] else "On Battery"
        return f"Power: {charging_str} ({batt})"

    def _toggle_charging_only(self, icon, item):
        config.ONLY_WHEN_CHARGING = not config.ONLY_WHEN_CHARGING
        logger.info(f"ONLY_WHEN_CHARGING set to {config.ONLY_WHEN_CHARGING}")

    def _build_menu(self) -> pystray.Menu:
        return pystray.Menu(
            pystray.MenuItem(lambda text: f"State: {self._state.value.title()}", None, enabled=False),
            pystray.MenuItem(lambda text: self._get_status_text(), None, enabled=False),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Hold Ctrl+Alt+Space to Talk", None, enabled=False),
            pystray.MenuItem("Trigger Kuro (4s listen)", lambda icon, item: self._on_trigger() if self._on_trigger else None),
            pystray.MenuItem(
                "Only Run When Charging",
                self._toggle_charging_only,
                checked=lambda item: config.ONLY_WHEN_CHARGING
            ),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Quit Kuro", lambda icon, item: self._quit())
        )

    def _quit(self):
        if self._icon:
            self._icon.stop()
        if self._on_quit:
            self._on_quit()

    def start(self):
        """Starts the system tray icon in a dedicated daemon thread."""
        initial_img = create_state_icon(KuroState.IDLE)
        self._icon = pystray.Icon(
            "Kuro",
            initial_img,
            "Kuro: Idle",
            menu=self._build_menu()
        )
        self._thread = threading.Thread(target=self._icon.run, daemon=True)
        self._thread.start()

    def stop(self):
        if self._icon:
            self._icon.stop()
