"""
Audit logging for SkillLite - records confirmation → execution → command full chain.

Events: confirmation_requested, confirmation_response, execution_started, execution_completed
Set SKILLLITE_AUDIT_LOG to a file path to enable (e.g. ~/.skilllite/audit/audit.jsonl).
"""

import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Optional


def _get_audit_path() -> Optional[Path]:
    """Get audit log path from env. Returns None if not configured."""
    path = os.environ.get("SKILLLITE_AUDIT_LOG") or os.environ.get("SKILLBOX_AUDIT_LOG")
    if not path:
        return None
    p = Path(path).expanduser()
    p.parent.mkdir(parents=True, exist_ok=True)
    return p


def _write_audit_event(event: str, **kwargs: Any) -> None:
    """Write a single audit event as JSONL. No-op if audit not enabled."""
    path = _get_audit_path()
    if not path:
        return
    record: Dict[str, Any] = {
        "ts": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "event": event,
        **{k: v for k, v in kwargs.items() if v is not None},
    }
    try:
        with open(path, "a", encoding="utf-8") as f:
            f.write(json.dumps(record, ensure_ascii=False) + "\n")
    except OSError:
        pass  # Silently ignore write errors (e.g. permission, disk full)


def audit_confirmation_requested(
    skill_id: str,
    code_hash: str,
    issues_count: int,
    severity: str,
    session_id: Optional[str] = None,
) -> None:
    """Emit when security scan finds high/critical issues and confirmation is needed."""
    _write_audit_event(
        event="confirmation_requested",
        skill_id=skill_id,
        code_hash=code_hash,
        issues_count=issues_count,
        severity=severity,
        session_id=session_id,
    )


def audit_confirmation_response(
    skill_id: str,
    approved: bool,
    source: str,  # "user" | "auto" | "cache"
    session_id: Optional[str] = None,
) -> None:
    """Emit when user/callback responds to confirmation prompt."""
    _write_audit_event(
        event="confirmation_response",
        skill_id=skill_id,
        approved=approved,
        source=source,
        session_id=session_id,
    )


def audit_execution_started(
    skill_id: str,
    entry_point: Optional[str] = None,
    session_id: Optional[str] = None,
) -> None:
    """Emit when skill execution is about to start."""
    _write_audit_event(
        event="execution_started",
        skill_id=skill_id,
        entry_point=entry_point,
        session_id=session_id,
    )


def audit_execution_completed(
    skill_id: str,
    exit_code: int,
    duration_ms: int,
    stdout_len: int = 0,
    success: bool = True,
    session_id: Optional[str] = None,
) -> None:
    """Emit when skill execution finishes."""
    _write_audit_event(
        event="execution_completed",
        skill_id=skill_id,
        exit_code=exit_code,
        duration_ms=duration_ms,
        stdout_len=stdout_len,
        success=success,
        session_id=session_id,
    )


__all__ = [
    "audit_confirmation_requested",
    "audit_confirmation_response",
    "audit_execution_started",
    "audit_execution_completed",
]
