"""Create a Google Meet from voice, open it in Edge, and send the link.

The Google login lives in %LOCALAPPDATA%\\Kuro\\google. This module does not
read any other project folder.
"""

from __future__ import annotations

import ctypes
import logging
import os
import re
import subprocess
import time
from typing import Any

logger = logging.getLogger(__name__)

NAME = "make_meet"
DESCRIPTION = (
    "Create a Google Meet, open it in Microsoft Edge, and send the link on WhatsApp. "
    "contact is the recipient name. Leave contact empty to only open the meeting."
)
PARAMETERS = {
    "type": "object",
    "properties": {
        "contact": {
            "type": "string",
            "description": "Who receives the link on WhatsApp. Empty if nobody.",
        }
    },
}

_SCOPES = ["https://www.googleapis.com/auth/meetings.space.created"]
_FILLERS = {"please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"}
# mate/meat/mete are the usual mishearings of Meet.
_MEET_WORD = r"(?:meet|mate|meat|mete)"
_PHRASE = re.compile(
    r"^(?:(?:make|create|start)\s+(?:a\s+|an\s+)?(?:google\s+)?"
    + _MEET_WORD
    + r"(?:\s+and\s+send(?:\s+(?:the|this|a))?(?:\s+link)?(?:\s+it)?\s+to\s+(?P<to>.+)"
    + r"|\s+for\s+(?P<forname>.+))?"
    + r"|send\s+(?:me\s+)?(?:the\s+|a\s+|my\s+)?(?:google\s+)?"
    + _MEET_WORD
    + r"(?:\s+link)?\s+to\s+(?P<sendto>.+))$"
)


def google_dir() -> str:
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return os.path.join(root, "Kuro", "google")


def _token_path() -> str:
    return os.path.join(google_dir(), "token.json")


def _normalize(text: str) -> str:
    text = (text or "").lower().strip()
    text = re.sub(r"[^a-z0-9\s]", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    toks = text.split()
    while toks and toks[0] in _FILLERS:
        toks = toks[1:]
    return " ".join(toks)


def _clean_name(name: str) -> str:
    name = re.sub(r"\s+(?:on|in|via)\s+whatsapp$", "", (name or "").strip())
    toks = name.split()
    while toks and toks[-1] in _FILLERS:
        toks.pop()
    return " ".join(toks)


def match_phrase(text: str) -> dict[str, str] | None:
    """Return {contact: name or ''} when this is a make-a-meet command."""
    found = _PHRASE.match(_normalize(text))
    if not found:
        return None
    name = _clean_name(found.group("to") or found.group("forname") or found.group("sendto") or "")
    return {"contact": name}


def _credentials():
    """Load the local Google login. None when it is missing or cannot refresh."""
    path = _token_path()
    if not os.path.isfile(path):
        return None
    try:
        from google.auth.transport.requests import Request
        from google.oauth2.credentials import Credentials

        creds = Credentials.from_authorized_user_file(path, _SCOPES)
        if creds and creds.expired and creds.refresh_token:
            creds.refresh(Request())
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(creds.to_json())
        if creds and creds.valid:
            return creds
    except Exception:
        logger.error("Google Meet login could not be read")
    return None


def _create_link(creds) -> str | None:
    try:
        from googleapiclient.discovery import build

        service = build("meet", "v2", credentials=creds, cache_discovery=False)
        space = service.spaces().create(body={}).execute()
        link = str(space.get("meetingUri") or "").strip()
        if link.startswith("https://"):
            logger.info("Google Meet created")
            return link
    except Exception:
        logger.error("Google Meet create failed")
    return None


def _copy_link(link: str) -> None:
    try:
        import pyperclip

        pyperclip.copy(link)
    except Exception:
        logger.info("Could not copy the Meet link")


def _edge_path() -> str | None:
    roots = [
        os.environ.get("PROGRAMFILES(X86)") or "",
        os.environ.get("PROGRAMFILES") or "",
        os.environ.get("LOCALAPPDATA") or "",
    ]
    for root in roots:
        path = os.path.join(root, "Microsoft", "Edge", "Application", "msedge.exe")
        if root and os.path.isfile(path):
            return path
    return None


def _edge_args(edge: str, link: str) -> list[str]:
    """A maximized app window, not a background tab in an existing window."""
    return [edge, f"--app={link}", "--start-maximized"]


def _list_windows() -> list[tuple[int, str, str]]:
    user32 = ctypes.windll.user32
    rows: list[tuple[int, str, str]] = []
    enum_proc = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)

    def _each(hwnd, _lparam):
        handle = int(hwnd or 0)
        if not handle or not user32.IsWindowVisible(handle):
            return True
        length = int(user32.GetWindowTextLengthW(handle) or 0)
        title = ""
        if length > 0:
            buf = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(handle, buf, length + 1)
            title = buf.value
        cls = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(handle, cls, 256)
        rows.append((handle, title, cls.value))
        return True

    callback = enum_proc(_each)
    user32.EnumWindows(callback, 0)
    return rows


def _is_edge_process(hwnd: int) -> bool:
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    kernel32.OpenProcess.restype = ctypes.c_void_p
    kernel32.OpenProcess.argtypes = [ctypes.c_uint, ctypes.c_bool, ctypes.c_ulong]
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.QueryFullProcessImageNameW.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint,
        ctypes.c_wchar_p,
        ctypes.POINTER(ctypes.c_ulong),
    ]
    kernel32.QueryFullProcessImageNameW.restype = ctypes.c_bool
    user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    pid = ctypes.c_ulong()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    if not pid.value:
        return False
    handle = kernel32.OpenProcess(0x1000, False, pid.value)
    if not handle:
        return False
    try:
        size = ctypes.c_ulong(512)
        buf = ctypes.create_unicode_buffer(512)
        if not kernel32.QueryFullProcessImageNameW(handle, 0, buf, ctypes.byref(size)):
            return False
        return buf.value.lower().endswith("\\msedge.exe")
    finally:
        kernel32.CloseHandle(handle)


def _window_area(hwnd: int) -> int:
    user32 = ctypes.windll.user32
    rect = ctypes.wintypes.RECT()
    if not user32.GetWindowRect(hwnd, ctypes.byref(rect)):
        return 0
    return max(0, int(rect.right) - int(rect.left)) * max(0, int(rect.bottom) - int(rect.top))


def _focus_hwnd(hwnd: int) -> bool:
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    fg = int(user32.GetForegroundWindow() or 0)
    current = int(kernel32.GetCurrentThreadId() or 0)
    other = int(user32.GetWindowThreadProcessId(fg, None) or 0) if fg else 0
    attached = False
    if other and other != current:
        attached = bool(user32.AttachThreadInput(current, other, True))
    try:
        user32.ShowWindow(hwnd, 3)
        user32.BringWindowToTop(hwnd)
        user32.SetForegroundWindow(hwnd)
    finally:
        if attached:
            user32.AttachThreadInput(current, other, False)
    return int(user32.GetForegroundWindow() or 0) == hwnd


def _wait_meet_window(before: dict[int, str], timeout: float) -> int | None:
    started = time.time()
    deadline = started + timeout
    while time.time() < deadline:
        chosen = 0
        best = 0
        for hwnd, title, cls in _list_windows():
            if cls != "Chrome_WidgetWin_1" or not _is_edge_process(hwnd):
                continue
            low = title.lower()
            is_new = hwnd not in before
            became_meet = (not is_new) and ("meet" in low) and ("meet" not in before.get(hwnd, "").lower())
            if not is_new and not became_meet:
                continue
            if "meet" not in low and (time.time() - started) < 1.5:
                continue
            area = _window_area(hwnd)
            if "meet" in low:
                area += 10_000_000
            if area > best:
                best = area
                chosen = hwnd
        if chosen:
            return chosen
        time.sleep(0.2)
    return None


def _click_join(hwnd: int, timeout: float) -> bool:
    """Press the Meet preview's Join now button. The page has no skip link."""
    script = r"""
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName UIAutomationClient
$root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]HWND)
if ($null -eq $root) { exit 3 }
$deadline = [DateTime]::UtcNow.AddSeconds(SECONDS)
$cond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::NameProperty, 'Join now')
while ([DateTime]::UtcNow -lt $deadline) {
  try {
    $el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
    if ($null -ne $el -and $el.Current.IsEnabled) {
      try {
        $pattern = $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        exit 0
      } catch {
        $legacy = $el.GetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern)
        $legacy.DoDefaultAction()
        exit 0
      }
    }
  } catch {}
  Start-Sleep -Milliseconds 400
}
exit 2
""".replace("HWND", str(int(hwnd))).replace("SECONDS", str(max(1, int(timeout))))
    try:
        done = subprocess.run(
            ["powershell", "-NoProfile", "-Sta", "-Command", script],
            capture_output=True,
            timeout=timeout + 10,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
    except Exception:
        logger.info("Meet join click failed")
        return False
    if done.returncode == 0:
        logger.info("Meet join clicked")
        return True
    logger.info("Meet join button was not clicked")
    return False


def _open_edge(link: str) -> bool:
    edge = _edge_path()
    if not edge:
        logger.error("Microsoft Edge was not found")
        return False
    before = {hwnd: title for hwnd, title, _cls in _list_windows()}
    try:
        subprocess.Popen(_edge_args(edge, link))
    except Exception:
        logger.error("Microsoft Edge did not open")
        return False
    hwnd = _wait_meet_window(before, 10.0)
    if not hwnd:
        logger.error("Meet window did not appear")
        return False
    _focus_hwnd(hwnd)
    _click_join(hwnd, 20.0)
    _focus_hwnd(hwnd)
    return True


def run(arguments: dict[str, Any] | None) -> str:
    try:
        args = arguments or {}
        contact = str(args.get("contact") or "").strip()
        name = ""
        digits = ""
        if contact:
            from tools.whatsapp import address_book, resolve_contact

            found = resolve_contact(contact, address_book())
            if not found:
                return "I don't know that contact."
            name, digits = found
        creds = _credentials()
        if creds is None:
            return "Google Meet is not signed in."
        link = _create_link(creds)
        if not link:
            return "I couldn't create the Google Meet."
        _copy_link(link)
        if not _open_edge(link):
            return "The Meet was created, but Edge did not open."
        if not contact:
            return "Success: The Meet is open."
        from tools.wa_link import deliver
        from tools.whatsapp import spoken_recipient

        sent = deliver(digits, text=link)
        if sent != "ok":
            return sent
        shown = spoken_recipient(name)
        logger.info("Meet link sent contact=%s", name)
        return f"Success: Sent the Meet link to {shown}."
    except Exception:
        logger.exception("make_meet failed")
        return "I couldn't create that Google Meet."
