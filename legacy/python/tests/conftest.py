"""Make legacy/python importable when pytest runs from the repo root."""

from __future__ import annotations

import sys
from pathlib import Path

_LEGACY = Path(__file__).resolve().parent.parent
_legacy = str(_LEGACY)
if _legacy not in sys.path:
    sys.path.insert(0, _legacy)
