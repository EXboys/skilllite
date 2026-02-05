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
    """Result of a security scan."""

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
        """Check if user confirmation is required."""
        return self.high_severity_count > 0

    def to_dict(self) -> Dict[str, Any]:
        return {
            "is_safe": self.is_safe,
            "issues": self.issues,
            "scan_id": self.scan_id,
            "code_hash": self.code_hash,
            "high_severity_count": self.high_severity_count,
            "medium_severity_count": self.medium_severity_count,
            "low_severity_count": self.low_severity_count,
            "requires_confirmation": self.requires_confirmation,
        }

    def format_report(self) -> str:
        """Format a human-readable security report."""
        if not self.issues:
            return "✅ Security scan passed. No issues found."

        lines = [
            f"📋 Security Scan Report (ID: {self.scan_id[:8]})",
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


class SecurityScanner:
    """
    Security scanner for skill execution.

    Uses skillbox binary to perform static code analysis before execution.

    This class supports singleton pattern for shared instance across
    all entry points (AgenticLoop, LangChain, LlamaIndex, MCP).
    """

    _instance: Optional["SecurityScanner"] = None

    @classmethod
    def get_instance(cls) -> "SecurityScanner":
        """Get singleton instance of the scanner."""
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    @classmethod
    def reset_instance(cls) -> None:
        """Reset singleton instance (for testing)."""
        cls._instance = None

    def __init__(self, skillbox_path: Optional[str] = None):
        """
        Initialize the security scanner.

        Args:
            skillbox_path: Path to skillbox binary. If None, will try to find it.
        """
        self._skillbox_path = skillbox_path
        self._scan_cache: Dict[str, SecurityScanResult] = {}
        self._SCAN_CACHE_TTL = 300  # 5 minutes

    @property
    def skillbox_path(self) -> Optional[str]:
        """Get skillbox binary path (lazy initialization)."""
        if self._skillbox_path is None:
            try:
                from ..sandbox.skillbox import find_binary
                self._skillbox_path = find_binary()
            except Exception:
                pass
        return self._skillbox_path

    def _generate_input_hash(self, skill_name: str, input_data: Dict[str, Any]) -> str:
        """Generate a hash of the input data for verification."""
        import json
        content = f"{skill_name}:{json.dumps(input_data, sort_keys=True, ensure_ascii=False)}"
        return hashlib.sha256(content.encode()).hexdigest()[:16]

    def _cleanup_expired_scans(self) -> None:
        """Remove expired scan results from cache."""
        current_time = time.time()
        expired_keys = [
            k for k, v in self._scan_cache.items()
            if current_time - v.timestamp > self._SCAN_CACHE_TTL
        ]
        for key in expired_keys:
            del self._scan_cache[key]

    def _parse_scan_output(self, output: str) -> List[Dict[str, Any]]:
        """Parse skillbox scan output into structured issues."""
        issues = []
        current_issue: Optional[Dict[str, Any]] = None

        for line in output.split('\n'):
            line = line.strip()
            if not line:
                continue

            # Detect severity markers
            if any(sev in line for sev in ['[Critical]', '[High]', '[Medium]', '[Low]']):
                if current_issue:
                    issues.append(current_issue)

                severity = "Medium"
                for sev in ['Critical', 'High', 'Medium', 'Low']:
                    if f'[{sev}]' in line:
                        severity = sev
                        break

                current_issue = {
                    "severity": severity,
                    "issue_type": "SecurityIssue",
                    "description": line,
                    "rule_id": "unknown",
                    "line_number": 0,
                    "code_snippet": "",
                }
            elif current_issue:
                # Try to extract line number
                if "line" in line.lower() and ":" in line:
                    try:
                        parts = line.split(":")
                        for part in parts:
                            if part.strip().isdigit():
                                current_issue["line_number"] = int(part.strip())
                                break
                    except (ValueError, IndexError):
                        pass
                # Append to description
                if current_issue["code_snippet"]:
                    current_issue["code_snippet"] += " " + line
                else:
                    current_issue["code_snippet"] = line

        if current_issue:
            issues.append(current_issue)

        return issues

    def scan_skill(
        self,
        skill_info: "SkillInfo",
        input_data: Dict[str, Any],
        entry_point: Optional[str] = None
    ) -> SecurityScanResult:
        """
        Perform a security scan on a skill before execution.

        Args:
            skill_info: SkillInfo object for the skill
            input_data: Input data for the skill execution
            entry_point: Optional specific entry point script

        Returns:
            SecurityScanResult with any issues found
        """
        self._cleanup_expired_scans()

        skill_name = skill_info.name
        input_hash = self._generate_input_hash(skill_name, input_data)
        scan_id = str(uuid.uuid4())

        # Determine entry point
        if entry_point:
            entry_script = skill_info.path / entry_point
        elif skill_info.metadata and skill_info.metadata.entry_point:
            entry_script = skill_info.path / skill_info.metadata.entry_point
        else:
            # Default entry points
            for default_entry in ["scripts/main.py", "main.py"]:
                entry_script = skill_info.path / default_entry
                if entry_script.exists():
                    break
            else:
                # No entry point found, return safe result
                return SecurityScanResult(
                    is_safe=True,
                    issues=[],
                    scan_id=scan_id,
                    code_hash=input_hash,
                )

        if not entry_script.exists():
            return SecurityScanResult(
                is_safe=True,
                issues=[],
                scan_id=scan_id,
                code_hash=input_hash,
            )

        # Use skillbox security-scan command
        if not self.skillbox_path:
            return SecurityScanResult(
                is_safe=True,
                issues=[],
                scan_id=scan_id,
                code_hash=input_hash,
            )

        try:
            result = subprocess.run(
                [self.skillbox_path, "security-scan", str(entry_script)],
                capture_output=True,
                text=True,
                timeout=30
            )

            # Parse scan output
            issues = self._parse_scan_output(result.stdout + result.stderr)
            high_count = sum(1 for i in issues if i.get("severity") in ["Critical", "High"])
            medium_count = sum(1 for i in issues if i.get("severity") == "Medium")
            low_count = sum(1 for i in issues if i.get("severity") == "Low")

            scan_result = SecurityScanResult(
                is_safe=high_count == 0,
                issues=issues,
                scan_id=scan_id,
                code_hash=input_hash,
                high_severity_count=high_count,
                medium_severity_count=medium_count,
                low_severity_count=low_count,
            )
            self._scan_cache[scan_id] = scan_result
            return scan_result

        except Exception:
            # On error, return safe result
            return SecurityScanResult(
                is_safe=True,
                issues=[],
                scan_id=scan_id,
                code_hash=input_hash,
            )

    def scan_code(
        self,
        language: str,
        code: str,
        sandbox_level: int = 3
    ) -> SecurityScanResult:
        """
        Perform a security scan on arbitrary code.

        This is used by MCP server to scan code before execution.

        Args:
            language: Programming language (python, javascript, etc.)
            code: Code to scan
            sandbox_level: Sandbox level (1, 2, or 3)

        Returns:
            SecurityScanResult with any issues found
        """
        import tempfile
        import os

        scan_id = str(uuid.uuid4())
        code_hash = hashlib.sha256(code.encode()).hexdigest()[:16]

        # Skip scanning for level 1/2
        if sandbox_level < 3:
            return SecurityScanResult(
                is_safe=True,
                issues=[],
                scan_id=scan_id,
                code_hash=code_hash,
            )

        if not self.skillbox_path:
            return SecurityScanResult(
                is_safe=True,
                issues=[],
                scan_id=scan_id,
                code_hash=code_hash,
            )

        # Determine file extension
        ext_map = {
            "python": ".py",
            "py": ".py",
            "javascript": ".js",
            "js": ".js",
            "bash": ".sh",
            "shell": ".sh",
        }
        ext = ext_map.get(language.lower(), ".txt")

        # Write code to temp file and scan
        try:
            with tempfile.NamedTemporaryFile(
                mode='w',
                suffix=ext,
                delete=False
            ) as f:
                f.write(code)
                temp_path = f.name

            try:
                result = subprocess.run(
                    [self.skillbox_path, "security-scan", temp_path],
                    capture_output=True,
                    text=True,
                    timeout=30
                )

                issues = self._parse_scan_output(result.stdout + result.stderr)
                high_count = sum(1 for i in issues if i.get("severity") in ["Critical", "High"])
                medium_count = sum(1 for i in issues if i.get("severity") == "Medium")
                low_count = sum(1 for i in issues if i.get("severity") == "Low")

                scan_result = SecurityScanResult(
                    is_safe=high_count == 0,
                    issues=issues,
                    scan_id=scan_id,
                    code_hash=code_hash,
                    high_severity_count=high_count,
                    medium_severity_count=medium_count,
                    low_severity_count=low_count,
                )
                self._scan_cache[scan_id] = scan_result
                return scan_result
            finally:
                os.unlink(temp_path)

        except Exception:
            return SecurityScanResult(
                is_safe=True,
                issues=[],
                scan_id=scan_id,
                code_hash=code_hash,
            )

    def get_cached_scan(self, scan_id: str) -> Optional[SecurityScanResult]:
        """Get a cached scan result by ID."""
        self._cleanup_expired_scans()
        return self._scan_cache.get(scan_id)

    def verify_scan(self, scan_id: str, code_hash: str) -> bool:
        """Verify that a scan ID matches the expected code hash."""
        cached = self.get_cached_scan(scan_id)
        if cached is None:
            return False
        return cached.code_hash == code_hash

