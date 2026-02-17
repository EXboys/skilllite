"""
Security scanning module for SkillLite core.

Provides SecurityScanResult and security scanning utilities that can be used
by SkillRunner, AgenticLoop, and adapters.

Usage:
    from skilllite.core.security import SecurityScanner, SecurityScanResult
    
    scanner = SecurityScanner()
    result = scanner.scan_skill(skill_info, input_data)
    
    if result.requires_confirmation:
        # Ask user for confirmation
        if confirmation_callback(result.format_report(), result.scan_id):
            # User confirmed, proceed with execution
            pass
"""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING
import hashlib
import json
import subprocess
import time
import uuid

if TYPE_CHECKING:
    from .skill_info import SkillInfo


# Type alias for confirmation callback
# Signature: (security_report: str, scan_id: str) -> bool
ConfirmationCallback = Callable[[str, str], bool]


@dataclass
class SecurityScanResult:
    """
    Unified security scan result — Single Source of Truth.

    Used by SecurityScanner, ipc_executor, BaseAdapter,
    LangChain/LlamaIndex adapters, and MCP server.

    Other modules should re-export this class rather than defining their own.
    """

    # Issue types that are HARD BLOCKED in L3 sandbox
    # (cannot execute even with user confirmation)
    HARD_BLOCKED_ISSUE_TYPES_L3 = {
        "Process Execution",
        "ProcessExecution",
        "process_execution",
    }

    # Rule IDs that are specifically hard blocked in L3 sandbox
    HARD_BLOCKED_RULE_IDS_L3 = {
        "py-subprocess",
        "py-os-system",
        "js-child-process",
    }

    # Dangerous module imports that lead to hard blocks
    HARD_BLOCKED_MODULES_L3 = {
        "py-os-import",
    }

    is_safe: bool
    issues: List[Dict[str, Any]] = field(default_factory=list)
    scan_id: str = ""
    code_hash: str = ""
    high_severity_count: int = 0
    medium_severity_count: int = 0
    low_severity_count: int = 0
    sandbox_level: int = 3
    timestamp: float = field(default_factory=time.time)

    # Computed in __post_init__ — not constructor args
    hard_blocked_issues: List[Dict[str, Any]] = field(
        default_factory=list, init=False, repr=False,
    )
    has_hard_blocked: bool = field(default=False, init=False, repr=False)

    def __post_init__(self) -> None:
        self.hard_blocked_issues = self._find_hard_blocked_issues()
        self.has_hard_blocked = len(self.hard_blocked_issues) > 0

    # ---- computed helpers ------------------------------------------------

    def _find_hard_blocked_issues(self) -> List[Dict[str, Any]]:
        """Find issues that are hard blocked in the current sandbox level."""
        if self.sandbox_level < 3:
            return []
        blocked = []
        for issue in self.issues:
            issue_type = issue.get("issue_type", "")
            rule_id = issue.get("rule_id", "")
            if (issue_type in self.HARD_BLOCKED_ISSUE_TYPES_L3
                    or rule_id in self.HARD_BLOCKED_RULE_IDS_L3):
                blocked.append(issue)
        return blocked

    @property
    def requires_confirmation(self) -> bool:
        """Check if user confirmation is required (high severity, not hard-blocked)."""
        return self.high_severity_count > 0 and not self.has_hard_blocked

    # ---- serialisation ---------------------------------------------------

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "is_safe": self.is_safe,
            "issues": self.issues,
            "scan_id": self.scan_id,
            "code_hash": self.code_hash,
            "high_severity_count": self.high_severity_count,
            "medium_severity_count": self.medium_severity_count,
            "low_severity_count": self.low_severity_count,
            "requires_confirmation": self.requires_confirmation,
            "has_hard_blocked": self.has_hard_blocked,
            "hard_blocked_count": len(self.hard_blocked_issues),
            "sandbox_level": self.sandbox_level,
            "timestamp": self.timestamp,
        }

    # ---- reporting -------------------------------------------------------

    def format_report(self) -> str:
        """Format a human-readable security report."""
        if not self.issues:
            return "✅ Security scan passed. No issues found."

        scan_id_display = self.scan_id[:8] if self.scan_id else "N/A"
        lines = [
            f"📋 Security Scan Report (ID: {scan_id_display})",
            f"   Sandbox Level: L{self.sandbox_level}",
            f"   Found {len(self.issues)} item(s) for review:",
            "",
        ]

        severity_icons = {
            "Critical": "🔴",
            "High": "🟠",
            "Medium": "🟡",
            "Low": "🟢",
        }

        for idx, issue in enumerate(self.issues, 1):
            severity = issue.get("severity", "Medium")
            icon = severity_icons.get(severity, "⚪")

            # Mark hard blocked issues
            is_hard_blocked = issue in self.hard_blocked_issues
            block_marker = " 🚫 [HARD BLOCKED]" if is_hard_blocked else ""

            lines.append(f"  {icon} #{idx} [{severity}] {issue.get('issue_type', 'Unknown')}{block_marker}")
            lines.append(f"     ├─ Rule: {issue.get('rule_id', 'N/A')}")
            lines.append(f"     ├─ Line {issue.get('line_number', '?')}: {issue.get('description', '')}")
            snippet = issue.get('code_snippet', '')
            lines.append(f"     └─ Code: {snippet[:60]}{'...' if len(snippet) > 60 else ''}")
            lines.append("")

        # Different messages based on hard-blocked status
        if self.has_hard_blocked:
            lines.append("🚫 HARD BLOCKED: This code contains operations that CANNOT be executed")
            lines.append(f"   in the current L{self.sandbox_level} sandbox environment.")
            lines.append("")
            lines.append("   The following operations are permanently blocked at runtime:")
            for issue in self.hard_blocked_issues:
                lines.append(f"   • {issue.get('issue_type', 'Unknown')}: {issue.get('description', '')}")
            lines.append("")
            lines.append("   ⚠️  Even with confirmation, this code will fail to execute.")
            lines.append("   Options:")
            lines.append("   1. Modify the code to remove blocked operations")
            lines.append("   2. Use a lower sandbox level (L1 or L2) if permitted")
        elif self.high_severity_count > 0:
            lines.append("⚠️  High severity issues found. Confirmation required to execute.")
            lines.append(f"   To proceed, call execute_code with confirmed=true and scan_id=\"{self.scan_id}\"")
        else:
            lines.append("ℹ️  Only low/medium severity issues found. Safe to execute.")

        return "\n".join(lines)

    # ---- factory methods -------------------------------------------------

    @classmethod
    def safe(cls, scan_id: str = "", code_hash: str = "") -> "SecurityScanResult":
        """Create a safe (no issues) scan result."""
        return cls(
            is_safe=True,
            issues=[],
            scan_id=scan_id,
            code_hash=code_hash,
        )

    @classmethod
    def from_issues(
        cls,
        issues: List[Dict[str, Any]],
        scan_id: str = "",
        code_hash: str = "",
        sandbox_level: int = 3,
    ) -> "SecurityScanResult":
        """Create a scan result from a list of issues."""
        high_count = sum(1 for i in issues if i.get("severity") in ["Critical", "High"])
        medium_count = sum(1 for i in issues if i.get("severity") == "Medium")
        low_count = sum(1 for i in issues if i.get("severity") == "Low")

        return cls(
            is_safe=high_count == 0,
            issues=issues,
            scan_id=scan_id,
            code_hash=code_hash,
            high_severity_count=high_count,
            medium_severity_count=medium_count,
            low_severity_count=low_count,
            sandbox_level=sandbox_level,
        )


def parse_scan_json_output(output: str) -> Dict[str, Any]:
    """Parse skillbox JSON scan output into structured result.

    This is a shared function used by SecurityScanner and adapters
    to parse the structured JSON output from `skillbox security-scan --json`.

    Args:
        output: Raw stdout from skillbox security-scan --json

    Returns:
        Dict with keys: is_safe, issues, high_severity_count,
        medium_severity_count, low_severity_count
    """
    try:
        data = json.loads(output)
        return {
            "is_safe": data.get("is_safe", True),
            "issues": data.get("issues", []),
            "high_severity_count": data.get("high_severity_count", 0),
            "medium_severity_count": data.get("medium_severity_count", 0),
            "low_severity_count": data.get("low_severity_count", 0),
        }
    except (json.JSONDecodeError, TypeError):
        # Fallback: if JSON parsing fails, return safe result
        return {
            "is_safe": True,
            "issues": [],
            "high_severity_count": 0,
            "medium_severity_count": 0,
            "low_severity_count": 0,
        }


def _run_skilllite_scan(binary_path: str, file_path: str) -> Dict[str, Any]:
    """Run skilllite security-scan --json on a file. Returns parsed data (fail-secure on error)."""
    fail_data = {
        "is_safe": False, "issues": [], "high_severity_count": 1,
        "medium_severity_count": 0, "low_severity_count": 0,
    }
    try:
        result = subprocess.run(
            [binary_path, "security-scan", "--json", file_path],
            capture_output=True, text=True, timeout=30,
        )
        if result.returncode != 0:
            fail_data["issues"] = [{
                "severity": "High", "issue_type": "Scan Error", "rule_id": "scan-error",
                "line_number": 0, "description": f"Scan failed (exit {result.returncode})",
                "code_snippet": (result.stderr or "")[:100],
            }]
            return fail_data
        return parse_scan_json_output(result.stdout)
    except subprocess.TimeoutExpired:
        fail_data["issues"] = [{
            "severity": "High", "issue_type": "Scan Timeout", "rule_id": "scan-timeout",
            "line_number": 0, "description": "Scan timed out", "code_snippet": "",
        }]
        return fail_data
    except Exception:
        fail_data["issues"] = [{
            "severity": "High", "issue_type": "Scan Error", "rule_id": "scan-exception",
            "line_number": 0, "description": "Scan error", "code_snippet": "",
        }]
        return fail_data


class SecurityScanner:
    """
    Security scanner — delegates to skilllite security-scan binary.
    """

    def __init__(self, skillbox_path: Optional[str] = None):
        self._binary = skillbox_path
        self._cache: Dict[str, SecurityScanResult] = {}
        self._cache_ttl = 300

    @property
    def skillbox_path(self) -> Optional[str]:
        if self._binary is None:
            try:
                from ..sandbox.core import find_binary
                self._binary = find_binary()
            except Exception:
                pass
        return self._binary

    def _cache_put(self, scan_id: str, result: SecurityScanResult) -> None:
        self._cache[scan_id] = result
        now = time.time()
        for k in [k for k, v in self._cache.items() if now - v.timestamp > self._cache_ttl]:
            del self._cache[k]

    def scan_skill(
        self,
        skill_info: "SkillInfo",
        input_data: Dict[str, Any],
        entry_point: Optional[str] = None
    ) -> SecurityScanResult:
        scan_id = str(uuid.uuid4())
        code_hash = hashlib.sha256(
            f"{skill_info.name}:{json.dumps(input_data, sort_keys=True, ensure_ascii=False)}".encode()
        ).hexdigest()[:16]

        entry_script = None
        if entry_point:
            entry_script = skill_info.path / entry_point
        elif skill_info.metadata and skill_info.metadata.entry_point:
            entry_script = skill_info.path / skill_info.metadata.entry_point
        else:
            for ep in ["scripts/main.py", "main.py"]:
                p = skill_info.path / ep
                if p.exists():
                    entry_script = p
                    break

        if not entry_script or not entry_script.exists() or not self.skillbox_path:
            return SecurityScanResult.safe(scan_id, code_hash)

        data = _run_skilllite_scan(self.skillbox_path, str(entry_script))
        r = SecurityScanResult(
            is_safe=data["is_safe"], issues=data["issues"], scan_id=scan_id, code_hash=code_hash,
            high_severity_count=data.get("high_severity_count", 0),
            medium_severity_count=data.get("medium_severity_count", 0),
            low_severity_count=data.get("low_severity_count", 0),
        )
        self._cache_put(scan_id, r)
        return r

    def scan_code(self, language: str, code: str, sandbox_level: int = 3) -> SecurityScanResult:
        import tempfile
        import os as _os
        scan_id = str(uuid.uuid4())
        code_hash = hashlib.sha256(code.encode()).hexdigest()[:16]
        if sandbox_level < 3 or not self.skillbox_path:
            return SecurityScanResult.safe(scan_id, code_hash)
        ext_map = {"python": ".py", "py": ".py", "javascript": ".js", "js": ".js", "bash": ".sh", "shell": ".sh"}
        suf = ext_map.get(language.lower(), ".txt")
        try:
            with tempfile.NamedTemporaryFile(mode='w', suffix=suf, delete=False) as f:
                f.write(code)
                tmp = f.name
            try:
                data = _run_skilllite_scan(self.skillbox_path, tmp)
                r = SecurityScanResult(
                    is_safe=data["is_safe"], issues=data["issues"], scan_id=scan_id, code_hash=code_hash,
                    high_severity_count=data.get("high_severity_count", 0),
                    medium_severity_count=data.get("medium_severity_count", 0),
                    low_severity_count=data.get("low_severity_count", 0),
                )
                self._cache_put(scan_id, r)
                return r
            finally:
                _os.unlink(tmp)
        except Exception:
            return SecurityScanResult.safe(scan_id, code_hash)

    def get_cached_scan(self, scan_id: str) -> Optional[SecurityScanResult]:
        now = time.time()
        for k in [k for k, v in self._cache.items() if now - v.timestamp > self._cache_ttl]:
            del self._cache[k]
        return self._cache.get(scan_id)

    def verify_scan(self, scan_id: str, code_hash: str) -> bool:
        c = self.get_cached_scan(scan_id)
        return c is not None and c.code_hash == code_hash

