"""Monochrome fluid HUD — bottom glass card, voice-reactive waveform."""

from __future__ import annotations

import math
import queue
import threading
import time
import tkinter as tk
from enum import Enum


class OverlayState(Enum):
    HIDDEN = "hidden"
    LISTENING = "listening"
    THINKING = "thinking"
    SPEAKING = "speaking"
    DORMANT = "dormant"
    ERROR = "error"
    READY = "ready"
    DONE = "done"


# Pure monochrome
_BG = "#0A0A0A"
_INK = "#F5F5F5"
_DIM = "#8C8C8C"
_MUTED = "#2A2A2A"
_WAVE = "#E8E8E8"
_SOFT = "#1A1A1A"
_ERR = "#FFFFFF"

_manager: "OverlayManager | None" = None


def current() -> "OverlayManager | None":
    """The running HUD, so other tools can share its window."""
    return _manager


def voice_level(samples) -> float:
    """Turn a mic block into a 0..1 bar height. Laptop speech is quiet at full scale."""
    import numpy as np

    audio = np.abs(np.asarray(samples, dtype=np.float32))
    if audio.size == 0:
        return 0.0
    peak = float(np.max(audio))
    if peak > 1.5:
        audio = audio / 32768.0
        peak = peak / 32768.0
    rms = float(np.sqrt(np.mean(np.square(audio))))
    raw = max(peak, rms * 3.0)
    if raw < 0.012:
        return min(1.0, raw * 2.0)
    return min(1.0, (raw - 0.012) * 16.0)


class OverlayManager:
    """Thread-safe bottom-center monochrome card with fluid mic waveform."""

    WIDTH = 500
    HEIGHT = 236
    WAVE_BARS = 56

    def __init__(self) -> None:
        self._q: queue.Queue = queue.Queue()
        self._thread: threading.Thread | None = None
        self._root: tk.Tk | None = None
        self._canvas: tk.Canvas | None = None

        self._state = OverlayState.HIDDEN
        self._status = ""
        self._you = ""
        self._kuro = ""
        self._tick = 0.0
        self._visible_until = 0.0
        self._levels: list[float] = [0.0] * self.WAVE_BARS
        self._smooth_levels: list[float] = [0.0] * self.WAVE_BARS
        self._level_lock = threading.Lock()
        self._rec_seconds = 0.0
        self._alpha = 0.0
        self._alpha_target = 0.0
        self._hiding = False
        self._lift = 24.0  # slide-up residual
        self._energy = 0.0  # smoothed peak for border pulse
        self._heard = False
        self._open_mic = False
        self._link = None
        self._link_holder = None
        self._link_photo = None

    def start(self) -> None:
        if self._thread and self._thread.is_alive():
            return
        global _manager
        _manager = self
        self._thread = threading.Thread(target=self._run, name="KuroOverlay", daemon=True)
        self._thread.start()
        for _ in range(100):
            if self._root is not None:
                break
            time.sleep(0.02)

    def stop(self) -> None:
        self._q.put(("stop", {}))

    def set_state(
        self,
        state: OverlayState | str,
        subtitle: str | None = None,
        hold_ms: int | None = None,
        listen_seconds: float | None = None,
    ) -> None:
        if isinstance(state, str):
            state = OverlayState(state.lower())
        self._q.put(("state", {"state": state, "subtitle": subtitle, "hold_ms": hold_ms}))

    def set_transcript(self, you: str | None = None, kuro: str | None = None) -> None:
        self._q.put(("transcript", {"you": you, "kuro": kuro}))

    def snapshot(self) -> dict:
        """State and 11 bar levels. No transcript text."""
        with self._level_lock:
            raw = list(self._levels)
        if not raw:
            bars = [0.0] * 11
        elif len(raw) <= 11:
            bars = raw + [0.0] * (11 - len(raw))
        else:
            step = len(raw) / 11
            bars = [raw[min(len(raw) - 1, int(i * step))] for i in range(11)]
        state = self._state.value if isinstance(self._state, OverlayState) else str(self._state)
        return {"state": state, "levels": [round(float(v), 3) for v in bars]}

    def transcript_pair(self) -> tuple[str, str]:
        return self._you or "", self._kuro or ""

    def push_level(self, level: float) -> None:
        level = max(0.0, min(1.0, float(level)))
        if level >= 0.08:
            self._heard = True
        with self._level_lock:
            self._levels.append(level)
            if len(self._levels) > self.WAVE_BARS:
                self._levels = self._levels[-self.WAVE_BARS :]

    def clear_levels(self) -> None:
        self._heard = False
        with self._level_lock:
            self._levels = [0.0] * self.WAVE_BARS
            self._smooth_levels = [0.0] * self.WAVE_BARS

    def set_open_mic(self, on: bool) -> None:
        self._open_mic = bool(on)

    def set_rec_seconds(self, seconds: float) -> None:
        self._q.put(("rec_seconds", {"seconds": seconds}))

    def force_hide(self) -> None:
        self._q.put(("force_hide", {}))

    def show_link_code(self, path: str | None) -> None:
        """Show or close the WhatsApp link code on this window's Tk loop."""
        self._q.put(("link_code", {"path": path}))

    def flash(self, title: str, subtitle: str = "", hold_ms: int = 2200, accent: str | None = None) -> None:
        state = (
            OverlayState.ERROR
            if any(w in title.lower() for w in ("wrong", "fail", "error", "no speech"))
            else OverlayState.READY
        )
        self.set_state(state, subtitle=subtitle or title, hold_ms=hold_ms)

    def _run(self) -> None:
        self._root = tk.Tk()
        self._root.title("Kuro")
        self._root.overrideredirect(True)
        self._root.attributes("-topmost", True)
        self._root.configure(bg=_BG)

        self._canvas = tk.Canvas(
            self._root, width=self.WIDTH, height=self.HEIGHT, bg=_BG, highlightthickness=0, bd=0
        )
        self._canvas.pack(fill="both", expand=True)

        self._position(extra_y=int(self._lift))
        self._apply_tool_window()
        try:
            self._root.attributes("-alpha", 0.01)
        except Exception:
            pass
        self._redraw()
        self._root.withdraw()

        self._root.after(16, self._pump)
        self._root.after(16, self._animate)
        self._root.mainloop()

    def _apply_tool_window(self) -> None:
        if self._root is None:
            return
        try:
            import ctypes

            self._root.update_idletasks()
            hwnd = self._root.winfo_id()
            GWL_EXSTYLE = -20
            WS_EX_TOOLWINDOW = 0x00000080
            user32 = ctypes.windll.user32
            style = user32.GetWindowLongW(hwnd, GWL_EXSTYLE)
            style |= WS_EX_TOOLWINDOW
            style &= ~0x00000020
            user32.SetWindowLongW(hwnd, GWL_EXSTYLE, style)
        except Exception:
            pass

    def _position(self, extra_y: int = 0) -> None:
        if self._root is None:
            return
        sw = self._root.winfo_screenwidth()
        sh = self._root.winfo_screenheight()
        x = max(0, (sw - self.WIDTH) // 2)
        y = max(0, sh - self.HEIGHT - 56 + extra_y)
        self._root.geometry(f"{self.WIDTH}x{self.HEIGHT}+{x}+{y}")

    def _set_alpha(self, value: float) -> None:
        if self._root is None:
            return
        try:
            self._root.attributes("-alpha", max(0.04, min(0.97, value)))
        except Exception:
            pass

    def _show(self) -> None:
        if self._root is None:
            return
        already = (not self._hiding) and self._alpha > 0.5 and self._root.state() != "withdrawn"
        self._hiding = False
        self._alpha_target = 0.96
        if already:
            return
        self._lift = 28.0
        self._position(extra_y=int(self._lift))
        self._root.deiconify()
        self._root.lift()
        self._root.attributes("-topmost", True)
        self._apply_tool_window()
        if self._alpha < 0.1:
            self._alpha = 0.12
            self._set_alpha(self._alpha)

    def _begin_hide(self) -> None:
        self._hiding = True
        self._alpha_target = 0.04

    def _finish_hide(self) -> None:
        if self._root is None:
            return
        self._state = OverlayState.HIDDEN
        self._hiding = False
        self._alpha = 0.0
        self._root.withdraw()

    def _wrap(self, text: str, width: int = 52) -> str:
        text = (text or "").strip()
        if not text:
            return ""
        words = text.split()
        lines: list[str] = []
        cur = ""
        for w in words:
            trial = f"{cur} {w}".strip()
            if len(trial) > width:
                if cur:
                    lines.append(cur)
                cur = w
            else:
                cur = trial
        if cur:
            lines.append(cur)
        if len(lines) > 3:
            lines = lines[:3]
            lines[-1] = lines[-1][: max(0, width - 3)] + "..."
        return "\n".join(lines)

    def _status_label(self) -> str:
        return {
            OverlayState.LISTENING: "listening",
            OverlayState.THINKING: "working",
            OverlayState.SPEAKING: "speaking",
            OverlayState.DORMANT: "needs power",
            OverlayState.ERROR: "error",
            OverlayState.READY: "ready",
            OverlayState.DONE: "done",
        }.get(self._state, "kuro")

    def _redraw(self) -> None:
        if self._canvas is None:
            return
        c = self._canvas
        c.delete("all")

        # Smooth mic levels toward targets (fluid, not jumpy)
        with self._level_lock:
            raw = list(self._levels)
        if len(self._smooth_levels) != len(raw):
            self._smooth_levels = list(raw)
        peak = 0.0
        for i, lvl in enumerate(raw):
            cur = self._smooth_levels[i] if i < len(self._smooth_levels) else 0.0
            # Attack fast, release slow
            if lvl > cur:
                cur = cur + (lvl - cur) * 0.55
            else:
                cur = cur + (lvl - cur) * 0.18
            self._smooth_levels[i] = cur
            peak = max(peak, cur)
        self._energy += (peak - self._energy) * 0.25

        pulse = 0.55 + 0.45 * self._energy
        if self._state == OverlayState.THINKING:
            pulse = 0.45 + 0.35 * abs(math.sin(self._tick / 18.0))
        elif self._state == OverlayState.SPEAKING:
            pulse = 0.6 + 0.4 * abs(math.sin(self._tick / 10.0))

        border_w = 1 + int(2 * pulse)
        c.create_rectangle(2, 2, self.WIDTH - 3, self.HEIGHT - 3, outline=_INK, width=border_w)
        c.create_rectangle(6, 6, self.WIDTH - 7, self.HEIGHT - 7, outline=_MUTED, width=1, fill=_BG)

        # Corner brackets — subtle motion
        span = 14 + int(6 * pulse)
        for x, y, dx, dy in (
            (10, 10, span, 0),
            (10, 10, 0, span),
            (self.WIDTH - 10, 10, -span, 0),
            (self.WIDTH - 10, 10, 0, span),
            (10, self.HEIGHT - 10, span, 0),
            (10, self.HEIGHT - 10, 0, -span),
            (self.WIDTH - 10, self.HEIGHT - 10, -span, 0),
            (self.WIDTH - 10, self.HEIGHT - 10, 0, -span),
        ):
            c.create_line(x, y, x + dx, y + dy, fill=_INK, width=2)

        c.create_text(20, 24, text="KURO", fill=_INK, font=("Segoe UI Semibold", 13), anchor="w")
        c.create_text(
            82, 25, text=f"// {self._status_label()}", fill=_DIM, font=("Segoe UI", 10), anchor="w"
        )

        status = self._status
        if self._state == OverlayState.LISTENING:
            if self._heard:
                status = "Hearing you"
            elif self._open_mic:
                status = "Go on"
            else:
                status = f"{self._rec_seconds:0.1f}s  ·  release to send"
        c.create_text(20, 46, text=status or "", fill=_DIM, font=("Segoe UI", 9), anchor="w", width=460)

        self._draw_waveform(20, 62, self.WIDTH - 40, 52)

        you = self._wrap(self._you)
        kuro = self._wrap(self._kuro)
        c.create_text(20, 128, text="YOU", fill=_DIM, font=("Segoe UI", 8), anchor="w")
        if you:
            hearing = you
        elif self._state == OverlayState.LISTENING and self._heard:
            hearing = "Hearing you"
        elif self._state == OverlayState.LISTENING:
            hearing = "…"
        else:
            hearing = "—"
        c.create_text(20, 144, text=hearing, fill=_INK, font=("Segoe UI", 11), anchor="nw", width=460)

        c.create_text(20, 180, text="KURO", fill=_DIM, font=("Segoe UI", 8), anchor="w")
        reply = kuro if kuro else (
            "…" if self._state in (OverlayState.THINKING, OverlayState.SPEAKING) else "—"
        )
        c.create_text(20, 196, text=reply, fill=_INK, font=("Segoe UI", 11), anchor="nw", width=460)

    def _draw_waveform(self, x: int, y: int, w: int, h: int) -> None:
        assert self._canvas is not None
        c = self._canvas
        c.create_rectangle(x, y, x + w, y + h, outline=_MUTED, width=1, fill=_SOFT)
        mid = y + h // 2
        # Baseline
        c.create_line(x + 4, mid, x + w - 4, mid, fill=_MUTED, width=1)

        levels = list(self._smooth_levels) if self._smooth_levels else [0.0] * self.WAVE_BARS
        n = max(1, len(levels))
        gap = 1
        bar_w = max(2, (w - 8 - gap * (n - 1)) // n)

        for i, lvl in enumerate(levels):
            # Voice-reactive with gentle idle motion
            amp = lvl
            if self._state == OverlayState.LISTENING:
                amp = min(1.0, max(amp * 1.35, 0.05 + 0.04 * abs(math.sin(self._tick / 14.0 + i * 0.2))))
                # Neighbor bleed for fluid look
                if i > 0:
                    amp = max(amp, levels[i - 1] * 0.35)
                if i + 1 < n:
                    amp = max(amp, levels[i + 1] * 0.35)
            elif self._state == OverlayState.SPEAKING:
                amp = 0.18 + 0.55 * abs(math.sin((self._tick + i * 2.2) / 6.0))
            elif self._state == OverlayState.THINKING:
                # Soft traveling pulse
                amp = 0.08 + 0.22 * abs(math.sin((self._tick * 0.08) - i * 0.35))
            else:
                amp *= 0.25

            bh = max(2, int((h - 12) * min(1.0, amp)))
            bx = x + 4 + i * (bar_w + gap)
            # Vertical gradient feel via two stacked rects
            c.create_rectangle(bx, mid - bh // 2, bx + bar_w, mid + bh // 2, outline="", fill=_WAVE)
            if bh > 8:
                c.create_rectangle(
                    bx, mid - bh // 2, bx + bar_w, mid - bh // 4, outline="", fill=_INK
                )

    def _apply_state(self, payload: dict) -> None:
        state: OverlayState = payload["state"]
        subtitle = payload.get("subtitle")
        hold_ms = payload.get("hold_ms")

        if state == OverlayState.HIDDEN:
            self._state = OverlayState.DONE
            if not self._status:
                self._status = "Done"
            self._visible_until = time.time() + 1.1
            self._show()
            return

        self._state = state
        if subtitle is not None:
            self._status = subtitle

        if state in (OverlayState.LISTENING, OverlayState.THINKING, OverlayState.SPEAKING):
            self._visible_until = 0.0
            if state == OverlayState.LISTENING:
                self._rec_seconds = 0.0
                self._heard = False
        else:
            default_hold = {
                OverlayState.DONE: 1200,
                OverlayState.READY: 2000,
                OverlayState.ERROR: 2400,
                OverlayState.DORMANT: 2800,
            }.get(state, 2200)
            hold = hold_ms if hold_ms is not None else default_hold
            self._visible_until = time.time() + (hold / 1000.0)

        self._show()

    def _pump(self) -> None:
        if self._root is None:
            return
        try:
            while True:
                kind, payload = self._q.get_nowait()
                if kind == "stop":
                    self._root.quit()
                    return
                if kind == "force_hide":
                    self._begin_hide()
                    continue
                if kind == "state":
                    self._apply_state(payload or {})
                elif kind == "transcript":
                    if payload.get("you") is not None:
                        self._you = payload["you"]
                    if payload.get("kuro") is not None:
                        self._kuro = payload["kuro"]
                    if self._state == OverlayState.HIDDEN:
                        self._show()
                elif kind == "rec_seconds":
                    self._rec_seconds = float((payload or {}).get("seconds") or 0.0)
                elif kind == "link_code":
                    self._apply_link((payload or {}).get("path"))
        except queue.Empty:
            pass

        if self._visible_until and time.time() >= self._visible_until:
            if self._state in (
                OverlayState.READY,
                OverlayState.DORMANT,
                OverlayState.ERROR,
                OverlayState.DONE,
            ):
                self._begin_hide()
                self._visible_until = 0.0

        self._root.after(16, self._pump)

    def _animate(self) -> None:
        if self._root is None:
            return

        # Ease alpha + slide. Leave the window where it landed so it does not flicker.
        self._alpha += (self._alpha_target - self._alpha) * 0.22
        self._lift += (0.0 - self._lift) * 0.18
        if abs(self._alpha - self._alpha_target) > 0.01:
            self._set_alpha(self._alpha)
        if self._lift > 0.5 and self._root.state() != "withdrawn":
            self._position(extra_y=int(self._lift))

        if self._state != OverlayState.HIDDEN and self._root.state() != "withdrawn":
            self._tick += 1.0
            self._redraw()

        if self._hiding and self._alpha < 0.08:
            if self._state in (OverlayState.LISTENING, OverlayState.THINKING, OverlayState.SPEAKING):
                self._hiding = False
                self._alpha_target = 0.96
            else:
                self._finish_hide()

        self._root.after(16, self._animate)

    def _apply_link(self, path: str | None) -> None:
        """WhatsApp link code on this Tk loop. A second Tk fights the HUD."""
        if self._root is None:
            return
        if not path:
            if self._link is not None:
                try:
                    self._link.destroy()
                except Exception:
                    pass
                self._link = None
                self._link_holder = None
                self._link_photo = None
            return
        try:
            from PIL import Image, ImageTk
        except Exception:
            return
        if self._link is None or not self._link.winfo_exists():
            win = tk.Toplevel(self._root)
            win.title("Link Kuro")
            win.attributes("-topmost", True)
            win.resizable(False, False)
            tk.Label(
                win,
                text="On your phone: WhatsApp, Linked devices, Link a device.\nScan this code. It refreshes on its own.",
                justify="left",
                padx=16,
                pady=12,
            ).pack()
            self._link_holder = tk.Label(win, bg="white")
            self._link_holder.pack(padx=16, pady=(0, 16))
            self._link = win
        image = Image.open(path).convert("RGB")
        image.thumbnail((360, 360))
        self._link_photo = ImageTk.PhotoImage(image, master=self._link)
        if self._link_holder is not None:
            self._link_holder.configure(image=self._link_photo, width=image.size[0], height=image.size[1])
