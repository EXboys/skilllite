"""Security scan result and reporting for MCP code execution."""

import time
from typing import Any, Dict, List


class SecurityScanResult:
    """Result of a security scan."""

    # Issue types that are HARD BLOCKED in L3 sandbox (cannot execute even with confirmation)
    # These operations are blocked at the sandbox runtime level, not just static analysis
    HARD_BLOCKED_ISSUE_TYPES_L3 = {
        "Process Execution",   # os.system, subprocess, etc.
        "ProcessExecution",    # Alternative format
        "process_execution",   # Snake case format
    }

    # Rule IDs that are specifically hard blocked in L3 sandbox
    HARD_BLOCKED_RULE_IDS_L3 = {
        "py-subprocess",       # subprocess.call/run/Popen
        "py-os-system",        # os.system/popen/spawn
        "js-child-process",    # child_process.exec/spawn
    }

    # Dangerous module imports that lead to hard blocks when combined with execution
    HARD_BLOCKED_MODULES_L3 = {
        "py-os-import",        # import os/subprocess/shutil
    }

    def __init__(
        self,
        is_safe: bool,
        issues: List[Dict[str, Any]],
        scan_id: str,
        code_hash: str,
        high_severity_count: int = 0,
        medium_severity_count: int = 0,
        low_severity_count: int = 0,
        sandbox_level: int = 3,
    ):
        self.is_safe = is_safe
        self.issues = issues
        self.scan_id = scan_id
        self.code_hash = code_hash
        self.high_severity_count = high_severity_count
        self.medium_severity_count = medium_severity_count
        self.low_severity_count = low_severity_count
        self.sandbox_level = sandbox_level
        self.timestamp = time.time()

        # Calculate hard blocked issues
        self.hard_blocked_issues = self._find_hard_blocked_issues()
        self.has_hard_blocked = len(self.hard_blocked_issues) > 0

    def _find_hard_blocked_issues(self) -> List[Dict[str, Any]]:
        """Find issues that are hard blocked in the current sandbox level."""
        if self.sandbox_level < 3:
            # Only L3 has hard blocks
            return []

        hard_blocked = []
        for issue in self.issues:
            issue_type = issue.get("issue_type", "")
            rule_id = issue.get("rule_id", "")

            # Check if this issue type or rule is hard blocked
            if (issue_type in self.HARD_BLOCKED_ISSUE_TYPES_L3 or
                rule_id in self.HARD_BLOCKED_RULE_IDS_L3):
                hard_blocked.append(issue)

        return hard_blocked

    def to_dict(self) -> Dict[str, Any]:
        return {
            "is_safe": self.is_safe,
            "issues": self.issues,
            "scan_id": self.scan_id,
            "code_hash": self.code_hash,
            "high_severity_count": self.high_severity_count,
            "medium_severity_count": self.medium_severity_count,
            "low_severity_count": self.low_severity_count,
            "requires_confirmation": self.high_severity_count > 0 and not self.has_hard_blocked,
            "has_hard_blocked": self.has_hard_blocked,
            "hard_blocked_count": len(self.hard_blocked_issues),
            "sandbox_level": self.sandbox_level,
        }

    def format_report(self) -> str:
        """Format a human-readable security report."""
        if not self.issues:
            return "✅ Security scan passed. No issues found."

        lines = [
            f"📋 Security Scan Report (ID: {self.scan_id[:8]})",
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
            lines.append(f"     └─ Code: {issue.get('code_snippet', '')[:60]}...")
            lines.append("")

        # Different messages based on whether there are hard blocked issues
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
