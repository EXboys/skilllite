"""
Unified type definitions for SkillLite adapters.

This module contains all shared types that are used across different
framework adapters (LangChain, LlamaIndex, MCP, etc.).

These types should be imported from here, not defined in individual adapters.
"""

import asyncio
import time
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional


# Type aliases for confirmation callbacks
ConfirmationCallback = Callable[[str, str], bool]
"""Synchronous confirmation callback: (security_report: str, scan_id: str) -> bool"""

AsyncConfirmationCallback = Callable[[str, str], "asyncio.Future[bool]"]
"""Asynchronous confirmation callback: (security_report: str, scan_id: str) -> Future[bool]"""


@dataclass
class ExecutionOptions:
    """Options for skill execution - shared across all adapters."""
    
    sandbox_level: int = 3
    """Sandbox security level (1/2/3). Default: 3 (full security)"""
    
    allow_network: bool = False
    """Whether to allow network access during execution."""
    
    timeout: Optional[int] = None
    """Execution timeout in seconds. None means use default."""
    
    confirmation_callback: Optional[ConfirmationCallback] = None
    """Callback for security confirmation (sync)."""
    
    async_confirmation_callback: Optional[AsyncConfirmationCallback] = None
    """Callback for security confirmation (async)."""


@dataclass
class SecurityScanResult:
    """
    Unified security scan result - used by all adapters.
    
    This is the single source of truth for security scan results.
    All adapters should import this class from here.
    
    Attributes:
        is_safe: Whether the code passed security checks
        issues: List of security issues found
        scan_id: Unique identifier for this scan
        code_hash: Hash of the scanned code
        high_severity_count: Number of high/critical severity issues
        medium_severity_count: Number of medium severity issues
        low_severity_count: Number of low severity issues
        timestamp: When the scan was performed
    """
    
    is_safe: bool
    issues: List[Dict[str, Any]] = field(default_factory=list)
    scan_id: str = ""
    code_hash: str = ""
    high_severity_count: int = 0
    medium_severity_count: int = 0
    low_severity_count: int = 0
    timestamp: float = field(default_factory=time.time)

    @property
    def requires_confirmation(self) -> bool:
        """Check if user confirmation is required (high severity issues found)."""
        return self.high_severity_count > 0

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
            "timestamp": self.timestamp,
        }

    def format_report(self) -> str:
        """Format a human-readable security report."""
        if not self.issues:
            return "✅ Security scan passed. No issues found."

        lines = [
            f"📋 Security Scan Report (ID: {self.scan_id[:8] if self.scan_id else 'N/A'})",
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
            lines.append(f"  {icon} #{idx} [{severity}] {issue.get('issue_type', 'Unknown')}")
            lines.append(f"     ├─ Rule: {issue.get('rule_id', 'N/A')}")
            lines.append(f"     ├─ Line {issue.get('line_number', '?')}: {issue.get('description', '')}")
            snippet = issue.get('code_snippet', '')
            lines.append(f"     └─ Code: {snippet[:60]}{'...' if len(snippet) > 60 else ''}")
            lines.append("")

        if self.high_severity_count > 0:
            lines.append("⚠️  High severity issues found. Confirmation required to execute.")
        else:
            lines.append("ℹ️  Only low/medium severity issues found. Safe to execute.")

        return "\n".join(lines)

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
        )


__all__ = [
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
    "ExecutionOptions",
]

