"""
Binary resolution: bundled first, then PATH.
"""

import os
import sys
from pathlib import Path
from typing import Optional

BINARY_NAME = "skilllite.exe" if sys.platform == "win32" else "skilllite"

_bundled_cache: Optional[str] = None


def get_bundled_binary() -> Optional[str]:
    """Return path to bundled skilllite binary, or None."""
    global _bundled_cache
    if _bundled_cache is not None:
        return _bundled_cache if _bundled_cache else None

    # skilllite/binary.py -> skilllite/bins/skilllite
    pkg_root = Path(__file__).resolve().parent
    bundled = pkg_root / "bins" / BINARY_NAME

    if bundled.exists() and os.access(bundled, os.X_OK):
        _bundled_cache = str(bundled)
        return _bundled_cache
    _bundled_cache = ""
    return None


def get_binary() -> Optional[str]:
    """Resolve binary: bundled first, then PATH."""
    bundled = get_bundled_binary()
    if bundled:
        return bundled
    import shutil
    return shutil.which(BINARY_NAME)
