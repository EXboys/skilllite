"""
Security event reporting - structured alerts when high-risk operations are intercepted.

Events: security_blocked, security_scan_high, security_scan_approved, security_scan_rejected
Set SKILLLITE_SECURITY_EVENTS_LOG to a file path to enable.
"""

import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional


def _get_events_path() -> Optional[Path]:
    """Get security events log path from env."""
    path = os.environ.get("SKILLLITE_SECURITY_EVENTS_LOG")
    if not path:
        return None
    p = Path(path).expanduser()
    p.parent.mkdir(parents=True, exist_ok=True)
    return p


def _write_security_event(event_type: str, category: str, skill_id: str, details: Dict[str, Any]) -> None:
    """Write a security event as JSONL."""
    path = _get_events_path()
    if not path:
        return
    record = {
        "ts": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "type": event_type,
        "category": category,
        "skill_id": skill_id,
        "details": details,
    }
    try:
        with open(path, "a", encoding="utf-8") as f:
            f.write(json.dumps(record, ensure_ascii=False) + "\n")
    except OSError:
        pass


def emit_security_scan_high(
    skill_id: str,
    severity: str,
    issues: List[Dict[str, Any]],
) -> None:
    """Emit when L3 scan finds High/Critical severity issues."""
    _write_security_event(
        event_type="security_scan_high",
        category="code_scan",
        skill_id=skill_id,
        details={"severity": severity, "issues": issues},
    )


def emit_security_scan_approved(skill_id: str, scan_id: str) -> None:
    """Emit when user approves execution after security review."""
    _write_security_event(
        event_type="security_scan_approved",
        category="code_scan",
        skill_id=skill_id,
        details={"scan_id": scan_id},
    )


def emit_security_scan_rejected(skill_id: str, scan_id: str) -> None:
    """Emit when user rejects execution after security review."""
    _write_security_event(
        event_type="security_scan_rejected",
        category="code_scan",
        skill_id=skill_id,
        details={"scan_id": scan_id},
    )


__all__ = [
    "emit_security_scan_high",
    "emit_security_scan_approved",
    "emit_security_scan_rejected",
]
