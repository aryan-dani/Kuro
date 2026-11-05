"""Send WhatsApp through a linked device in the background.

Contacts live outside the repo. The desktop app is not opened.
"""

from __future__ import annotations

import ctypes
import logging
import os
import re
import time
from typing import Any

import cancel

logger = logging.getLogger(__name__)

NAME = "send_whatsapp"
DESCRIPTION = (
    "Send a WhatsApp message through the linked device to a contact saved in the local contacts file. "
    "source is clipboard, browser_url, or text. Never guess a recipient."
)
PARAMETERS = {
    "type": "object",
    "properties": {
        "contact": {
            "type": "string",
            "description": "Contact name from the local contacts file. Use me when the user did not name someone else.",
        },
        "source": {
            "type": "string",
            "enum": ["clipboard", "browser_url", "text"],
            "description": "Where the message body comes from",
        },
        "text": {"type": "string", "description": "Spoken message when source is text"},
    },
    "required": ["contact", "source"],
}

_CUTOFF = 0.75
_URL_RE = re.compile(r"^https?://\S+$", re.IGNORECASE)
_FILLERS = {"please", "hey", "kuro", "can", "you", "could", "would", "just", "kindly"}

_KIND = r"meeting link|link|clipboard|message"
_PHRASE = re.compile(
    rf"^(?:send\s+(?:me\s+)?(?:(?:my|the|this)\s+)?(?P<kind>{_KIND})\s+to\s+(?P<name>.+?)\s+(?:on|in|via)\s+whatsapp"
    rf"|send\s+(?:me\s+)?(?:(?:my|the|this)\s+)?(?P<kind_me>{_KIND})(?:\s+(?!to\b)\w+){{0,4}}\s+(?:on|in|via)\s+whatsapp"
    r"|whatsapp\s+(?P<name2>.+?)\s+(?:this|the)\s+link)$"
)


def contacts_path() -> str:
    root = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return os.path.join(root, "Kuro", "contacts.json")


def normalize(text: str) -> str:
    text = (text or "").lower().strip()
    text = re.sub(r"[^a-z0-9\s]", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    return text


def clean_digits(raw: str) -> str:
    return re.sub(r"\D", "", raw or "")


def url_ok(value: str) -> bool:
    return bool(_URL_RE.match((value or "").strip()))


def _jid_digits(jid: str) -> str:
    raw = (jid or "").strip().lower()
    if "@" not in raw:
        return ""
    user, host = raw.split("@", 1)
    if not host.startswith("s.whatsapp.net"):
        return ""
    user = user.split(":", 1)[0]
    digits = clean_digits(user)
    if len(digits) < 8 or digits != user:
        return ""
    return digits


def load_wa_contacts() -> dict[str, str]:
    """Names from the linked WhatsApp account. Ambiguous names are left out."""
    import sqlite3

    from tools.wa_link import session_path

    path = session_path()
    if not os.path.isfile(path):
        return {}
    grouped: dict[str, tuple[str, set[str]]] = {}
    try:
        uri = "file:///" + path.replace("\\", "/") + "?mode=ro"
        con = sqlite3.connect(uri, uri=True, timeout=1.0)
        try:
            rows = con.execute(
                "SELECT first_name, full_name, push_name, business_name, their_jid "
                "FROM whatsmeow_contacts"
            )
            for first, full, push, business, jid in rows:
                digits = _jid_digits(str(jid or ""))
                if not digits:
                    continue
                for raw in (full, push, first, business):
                    name = str(raw or "").strip()
                    key = normalize(name)
                    if not key:
                        continue
                    display, numbers = grouped.get(key, (name, set()))
                    numbers.add(digits)
                    grouped[key] = (display, numbers)
        finally:
            con.close()
    except Exception:
        logger.info("WhatsApp contacts unavailable")
        return {}
    book: dict[str, str] = {}
    for _key, (display, numbers) in grouped.items():
        if len(numbers) == 1:
            book[display] = next(iter(numbers))
    return book


def address_book() -> dict[str, str]:
    """Saved contacts first, then names from the linked WhatsApp account."""
    book = dict(load_contacts())
    taken = {normalize(name) for name in book}
    for name, digits in load_wa_contacts().items():
        key = normalize(name)
        if key and key not in taken:
            book[name] = digits
            taken.add(key)
    return book


def load_contacts() -> dict[str, str]:
    """Name -> digits. Never raises."""
    path = contacts_path()
    try:
        import json

        with open(path, encoding="utf-8") as f:
            data = json.load(f)
    except FileNotFoundError:
        return {}
    except Exception:
        logger.warning("Contacts file unreadable")
        return {}
    if not isinstance(data, dict):
        return {}
    book: dict[str, str] = {}
    for name, number in data.items():
        digits = clean_digits(str(number))
        if str(name).strip() and len(digits) >= 8:
            book[str(name).strip()] = digits
    return book


def _edit_distance(a: str, b: str) -> int:
    """Distance, or 2 once it is already too far to be a one-letter miss."""
    if abs(len(a) - len(b)) > 1:
        return 2
    if len(a) == len(b):
        return sum(x != y for x, y in zip(a, b))
    longer, shorter = (a, b) if len(a) > len(b) else (b, a)
    i = j = skipped = 0
    while i < len(longer) and j < len(shorter):
        if longer[i] != shorter[j]:
            skipped += 1
            if skipped > 1:
                return 2
            i += 1
            continue
        i += 1
        j += 1
    return 1


def _shared_prefix(a: str, b: str) -> int:
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return n


def _one_letter_miss(query_n: str, book: dict[str, str]) -> tuple[str, str] | None:
    """A unique one-letter mishear, such as mob for Mom. Ties are refused."""
    found: dict[str, tuple[str, str]] = {}
    for name, digits in book.items():
        key = normalize(name)
        if key and _edit_distance(query_n, key) == 1:
            found.setdefault(key, (name, digits))
    if not found:
        return None
    if len(found) == 1:
        return next(iter(found.values()))
    ranked = sorted(found.items(), key=lambda item: _shared_prefix(query_n, item[0]), reverse=True)
    best_n = _shared_prefix(query_n, ranked[0][0])
    next_n = _shared_prefix(query_n, ranked[1][0])
    if best_n >= 2 and best_n > next_n:
        return ranked[0][1]
    return None


def resolve_contact(query: str, book: dict[str, str] | None = None) -> tuple[str, str] | None:
    """Return (display_name, digits) or None. Never guesses a distant name."""
    import difflib

    book = address_book() if book is None else book
    query_n = normalize(query)
    if not query_n or not book:
        return None
    for name, digits in book.items():
        if normalize(name) == query_n:
            return name, digits
    choices = list(book.keys())
    hits = difflib.get_close_matches(query, choices, n=1, cutoff=_CUTOFF)
    if not hits:
        hits = difflib.get_close_matches(query_n, [normalize(n) for n in choices], n=1, cutoff=_CUTOFF)
        if not hits:
            return _one_letter_miss(query_n, book)
        for name, digits in book.items():
            if normalize(name) == hits[0]:
                return name, digits
        return None
    return hits[0], book[hits[0]]


def _strip_fillers(text: str) -> str:
    toks = normalize(text).split()
    while toks and toks[0] in _FILLERS:
        toks = toks[1:]
    return " ".join(toks)


def _source_for(kind: str) -> str:
    if kind in ("link", "meeting link"):
        return "browser_url"
    if kind == "clipboard":
        return "clipboard"
    return "clipboard"


def match_phrase(text: str) -> dict[str, str] | None:
    body = _strip_fillers(text)
    found = _PHRASE.match(body)
    if not found:
        return None
    groups = found.groupdict()
    plain = groups.get("kind_me") or groups.get("kind_plain")
    if plain:
        return {"contact": "me", "source": _source_for(plain)}
    kind = found.group("kind")
    name = (found.group("name") or found.group("name2") or "").strip()
    if not name:
        return None
    if kind:
        source = _source_for(kind)
    else:
        source = "browser_url"
    return {"contact": name, "source": source}


def prompt_fragment() -> str:
    """Whisper bias from contact names. Never raises."""
    try:
        names = list(load_contacts().keys())
        if not names:
            return ""
        joined = ", ".join(names[:12])
        return f" Send this link to {joined} on WhatsApp."
    except Exception:
        return ""


def _display(name: str) -> str:
    cleaned = name.strip()
    if not cleaned:
        return "them"
    return cleaned[0].upper() + cleaned[1:]


def spoken_recipient(name: str) -> str:
    """How to say the contact. The saved name "me" is spoken as "you"."""
    if normalize(name) == "me":
        return "you"
    return _display(name)


def target_is_front(foreground: int, target: int) -> bool:
    """True only when the keystrokes would land in the intended window."""
    return bool(target) and int(foreground) == int(target)


def _is_whatsapp_title(title: str) -> bool:
    text = (title or "").strip().lower()
    if not text or any(skip in text for skip in ("chrome", "edge", "firefox", "http")):
        return False
    return text == "whatsapp" or text.endswith(") whatsapp")


def _wait(seconds: float) -> bool:
    """True when the user cancelled."""
    end = time.time() + seconds
    while time.time() < end:
        if cancel.event.is_set():
            return True
        time.sleep(0.02)
    return cancel.event.is_set()


_IMAGE_FORMATS = {2, 8, 17, 15}  # bitmap, dib, dibv5, file drop


def payload_kind(text: str, formats: set[int]) -> str:
    """text, image, or empty. formats are Windows clipboard format ids."""
    if (text or "").strip():
        return "text"
    if formats & _IMAGE_FORMATS:
        return "image"
    return "empty"


def _clipboard_formats() -> set[int]:
    import ctypes

    user32 = ctypes.windll.user32
    if not user32.OpenClipboard(None):
        return set()
    found: set[int] = set()
    try:
        fmt = 0
        while True:
            fmt = user32.EnumClipboardFormats(fmt)
            if not fmt:
                break
            found.add(int(fmt))
    finally:
        user32.CloseClipboard()
    return found


def _clipboard_text() -> str:
    import pyperclip

    return str(pyperclip.paste() or "").strip()


def _clipboard_image_bytes() -> bytes | None:
    """PNG bytes from the clipboard image, or None. Never logs the bytes."""
    try:
        from io import BytesIO

        from PIL import Image, ImageGrab
    except Exception:
        return None
    try:
        grabbed = ImageGrab.grabclipboard()
    except Exception:
        logger.info("Clipboard image read failed")
        return None
    image = None
    if isinstance(grabbed, Image.Image):
        image = grabbed
    elif isinstance(grabbed, list) and grabbed:
        try:
            image = Image.open(grabbed[0])
        except Exception:
            return None
    if image is None:
        return None
    out = BytesIO()
    image.convert("RGB").save(out, format="PNG")
    data = out.getvalue()
    return data or None


def _browser_url() -> str:
    import pyautogui
    import pyperclip

    previous = ""
    try:
        previous = str(pyperclip.paste() or "")
    except Exception:
        previous = ""
    pyautogui.FAILSAFE = False
    cancel.arm_synthetic(1.5)
    pyautogui.hotkey("ctrl", "l")
    time.sleep(0.25)
    cancel.arm_synthetic(1.5)
    pyautogui.hotkey("ctrl", "c")
    time.sleep(0.25)
    copied = str(pyperclip.paste() or "").strip()
    try:
        pyperclip.copy(previous)
    except Exception:
        logger.info("Could not restore clipboard length=%s", len(previous))
    return copied


def _user32():
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    user32.GetForegroundWindow.restype = ctypes.c_void_p
    user32.SetForegroundWindow.argtypes = [ctypes.c_void_p]
    user32.SetForegroundWindow.restype = ctypes.c_bool
    user32.BringWindowToTop.argtypes = [ctypes.c_void_p]
    user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
    user32.IsWindowVisible.restype = ctypes.c_bool
    user32.IsIconic.argtypes = [ctypes.c_void_p]
    user32.IsIconic.restype = ctypes.c_bool
    user32.ShowWindow.argtypes = [ctypes.c_void_p, ctypes.c_int]
    user32.GetWindowTextLengthW.argtypes = [ctypes.c_void_p]
    user32.GetWindowTextLengthW.restype = ctypes.c_int
    user32.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
    user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    user32.GetWindowThreadProcessId.restype = ctypes.c_ulong
    user32.AttachThreadInput.argtypes = [ctypes.c_ulong, ctypes.c_ulong, ctypes.c_bool]
    user32.AttachThreadInput.restype = ctypes.c_bool
    kernel32.GetCurrentThreadId.restype = ctypes.c_ulong
    return user32, kernel32


def _window_title(user32, hwnd: int) -> str:
    length = int(user32.GetWindowTextLengthW(hwnd) or 0)
    if length <= 0:
        return ""
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buf, length + 1)
    return buf.value


def _foreground(user32) -> int:
    return int(user32.GetForegroundWindow() or 0)


def _find_whatsapp(user32) -> int:
    found: list[int] = []
    enum_proc = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)

    def _each(hwnd, _lparam):
        handle = int(hwnd or 0)
        if handle and user32.IsWindowVisible(handle) and _is_whatsapp_title(_window_title(user32, handle)):
            found.append(handle)
        return True

    callback = enum_proc(_each)
    user32.EnumWindows(callback, 0)
    return found[0] if found else 0


def _focus(user32, kernel32, hwnd: int) -> bool:
    """Put hwnd in front. Returns True only after Windows agrees."""
    if not hwnd:
        return False
    if user32.IsIconic(hwnd):
        user32.ShowWindow(hwnd, 9)
    fg = _foreground(user32)
    current = int(kernel32.GetCurrentThreadId() or 0)
    other = int(user32.GetWindowThreadProcessId(fg, None) or 0) if fg else 0
    attached = False
    if other and other != current:
        attached = bool(user32.AttachThreadInput(current, other, True))
    try:
        user32.BringWindowToTop(hwnd)
        user32.SetForegroundWindow(hwnd)
    finally:
        if attached:
            user32.AttachThreadInput(current, other, False)
    return target_is_front(_foreground(user32), hwnd)


def _restore_focus(user32, kernel32, hwnd: int) -> None:
    if hwnd and not _focus(user32, kernel32, hwnd):
        logger.info("Could not return to the previous window")


def _message_for(source: str, text: str) -> str | None:
    """Return the body, or None when it should be refused. Raises only on import failure."""
    if source == "text":
        body = (text or "").strip()
        return body or None
    if source == "clipboard":
        return _clipboard_text() or None
    if source == "browser_url":
        copied = _browser_url()
        if url_ok(copied):
            return copied
        return None
    return None


def run(arguments: dict[str, Any] | None) -> str:
    try:
        args = arguments or {}
        contact = str(args.get("contact") or "").strip()
        source = str(args.get("source") or "").strip().lower()
        text = str(args.get("text") or "")
        if source not in ("clipboard", "browser_url", "text"):
            return "I need clipboard, a browser link, or spoken text."
        book = address_book()
        if not book:
            return "No contacts are configured."
        found = resolve_contact(contact, book)
        if not found:
            return "I don't know that contact."
        name, digits = found
        shown = spoken_recipient(name)
        try:
            body = _message_for(source, text)
            kind = "text" if body else "empty"
            if source == "clipboard" and not body:
                kind = payload_kind("", _clipboard_formats())
        except Exception:
            logger.exception("WhatsApp content read failed")
            return "I couldn't read that message."
        if source == "browser_url" and not body:
            return "I could not copy a link from the browser."
        image = None
        if source == "clipboard" and kind == "image":
            image = _clipboard_image_bytes()
            if not image:
                kind = "empty"
        if source == "clipboard" and kind == "empty":
            return "The clipboard is empty. Copy text or an image first."
        if source != "clipboard" and not body:
            return "There is nothing to send."
        logger.info("WhatsApp prepare contact=%s source=%s kind=%s length=%s", name, source, kind, len(body or "") if not image else len(image))
        from tools.wa_link import deliver

        result = deliver(digits, text=body or "", image=image if kind == "image" else None)
        if result == "ok":
            logger.info("WhatsApp sent contact=%s kind=%s", name, kind)
            return f"Success: Sent to {shown}."
        return result
    except Exception:
        logger.exception("send_whatsapp failed")
        return "I couldn't send that WhatsApp."
