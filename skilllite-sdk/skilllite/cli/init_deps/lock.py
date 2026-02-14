"""Lock file I/O for .skilllite.lock."""

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional


LOCK_FILE_NAME = ".skilllite.lock"


def read_lock_file(skill_dir: Path) -> Optional[Dict[str, Any]]:
    """Read and return the parsed ``.skilllite.lock`` for *skill_dir*."""
    lock_path = skill_dir / LOCK_FILE_NAME
    if not lock_path.exists():
        return None
    try:
        return json.loads(lock_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def write_lock_file(
    skill_dir: Path,
    compatibility: Optional[str],
    language: str,
    packages: List[str],
    resolver: str = "whitelist",
) -> None:
    """Write a ``.skilllite.lock`` file into *skill_dir*."""
    compat_hash = hashlib.sha256(
        (compatibility or "").encode()
    ).hexdigest()
    lock_data = {
        "compatibility_hash": compat_hash,
        "language": language,
        "resolved_packages": sorted(packages),
        "resolved_at": datetime.now(timezone.utc).isoformat(),
        "resolver": resolver,
    }
    lock_path = skill_dir / LOCK_FILE_NAME
    lock_path.write_text(
        json.dumps(lock_data, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def lock_is_stale(
    lock_data: Dict[str, Any],
    compatibility: Optional[str],
) -> bool:
    """Return ``True`` if *lock_data* does not match the current compatibility."""
    current_hash = hashlib.sha256(
        (compatibility or "").encode()
    ).hexdigest()
    return lock_data.get("compatibility_hash") != current_hash
