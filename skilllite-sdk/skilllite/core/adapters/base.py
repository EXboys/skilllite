"""
Base Adapter - Common logic for all framework adapters.

This module provides the BaseAdapter class which implements shared functionality
for all framework integrations (LangChain, LlamaIndex, MCP, etc.).

All adapters should inherit from BaseAdapter to ensure consistent behavior
and reduce code duplication.

Key Features:
1. Unified execution through UnifiedExecutionService
2. Shared security scanning logic
3. Common confirmation flow
4. Consistent error handling
"""

import hashlib
import json
import os
import subprocess
import time
import uuid
from abc import ABC, abstractmethod
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

from ..protocols import (
    SecurityScanResult,
    ConfirmationCallback,
    AsyncConfirmationCallback,
    ExecutionOptions,
)

if TYPE_CHECKING:
    from ..manager import SkillManager
    from ..skill_info import SkillInfo
    from ...sandbox.base import ExecutionResult


class BaseAdapter(ABC):
    """
    Base class for all framework adapters.
    
    Implements common logic for:
    - Skill execution via UnifiedExecutionService
    - Security scanning
    - Confirmation flow
    - Caching
    
    Subclasses only need to implement framework-specific tool creation.
    """
    
    # Class-level caches (shared across instances)
    _scan_cache: Dict[str, SecurityScanResult] = {}
    _confirmed_skills: Dict[str, float] = {}
    _SCAN_CACHE_TTL: int = 300  # 5 minutes
    _CONFIRMATION_TTL: int = 3600  # 1 hour
    
    def __init__(
        self,
        manager: "SkillManager",
        sandbox_level: int = 3,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        confirmation_callback: Optional[ConfirmationCallback] = None,
        async_confirmation_callback: Optional[AsyncConfirmationCallback] = None,
        skill_names: Optional[List[str]] = None,
    ):
        """
        Initialize the adapter.
        
        Args:
            manager: SkillManager instance with registered skills
            sandbox_level: Sandbox security level (1/2/3, default: 3)
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            confirmation_callback: Sync callback for security confirmation
            async_confirmation_callback: Async callback for security confirmation
            skill_names: Optional list of skill names to include (default: all)
        """
        self.manager = manager
        self.sandbox_level = sandbox_level
        self.allow_network = allow_network
        self.timeout = timeout
        self.confirmation_callback = confirmation_callback
        self.async_confirmation_callback = async_confirmation_callback
        self.skill_names = skill_names
    
    def execute_skill(
        self,
        skill_name: str,
        input_data: Dict[str, Any],
    ) -> "ExecutionResult":
        """
        Execute a skill using UnifiedExecutionService.
        
        This is the unified execution entry point that all adapters should use.
        It handles security scanning, confirmation, and execution.
        
        Args:
            skill_name: Name of the skill to execute
            input_data: Input data for the skill
            
        Returns:
            ExecutionResult with output or error
        """
        from ...sandbox.execution_service import UnifiedExecutionService
        from ...sandbox.base import ExecutionResult
        
        skill_info = self.manager.get_skill(skill_name)
        if not skill_info:
            return ExecutionResult(
                success=False,
                error=f"Skill '{skill_name}' not found",
                exit_code=1,
            )
        
        service = UnifiedExecutionService.get_instance()
        return service.execute_skill(
            skill_info=skill_info,
            input_data=input_data,
            confirmation_callback=self.confirmation_callback,
            allow_network=self.allow_network,
            timeout=self.timeout,
        )
    
    def get_executable_skills(self) -> List["SkillInfo"]:
        """Get list of executable skills, optionally filtered by skill_names."""
        skills = self.manager.list_executable_skills()
        if self.skill_names:
            skills = [s for s in skills if s.name in self.skill_names]
        return skills
    
    @abstractmethod
    def to_tools(self) -> List[Any]:
        """
        Convert skills to framework-specific tools.
        
        Subclasses must implement this to create tools for their framework.
        
        Returns:
            List of framework-specific tool objects
        """
        pass
    
    # ==================== Security Scanning ====================
    
    def _generate_input_hash(self, input_data: Dict[str, Any]) -> str:
        """Generate a hash of the input data for cache key."""
        content = json.dumps(input_data, sort_keys=True, ensure_ascii=False)
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

    def perform_security_scan(
        self,
        skill_name: str,
        input_data: Dict[str, Any],
    ) -> SecurityScanResult:
        """
        Perform a security scan on the skill.

        Uses skillbox binary for actual scanning.
        Results are cached for performance.

        Args:
            skill_name: Name of the skill to scan
            input_data: Input data for the skill

        Returns:
            SecurityScanResult with scan results
        """
        self._cleanup_expired_scans()
        input_hash = self._generate_input_hash(input_data)
        scan_id = str(uuid.uuid4())

        try:
            skill_info = self.manager.get_skill(skill_name)
            if not skill_info:
                return SecurityScanResult.safe(scan_id, input_hash)

            entry_point = None
            if skill_info.metadata:
                entry_point = getattr(skill_info.metadata, 'entry_point', None)

            if skill_info and entry_point:
                entry_script = skill_info.path / entry_point
                if entry_script.exists():
                    from ...sandbox.skillbox import find_binary

                    skillbox_path = find_binary()
                    if skillbox_path:
                        result = subprocess.run(
                            [skillbox_path, "security-scan", str(entry_script)],
                            capture_output=True,
                            text=True,
                            timeout=30
                        )
                        issues = self._parse_scan_output(result.stdout + result.stderr)
                        scan_result = SecurityScanResult.from_issues(
                            issues=issues,
                            scan_id=scan_id,
                            code_hash=input_hash,
                        )
                        self._scan_cache[scan_id] = scan_result
                        return scan_result
        except Exception:
            pass

        # Default: no issues found
        scan_result = SecurityScanResult.safe(scan_id, input_hash)
        self._scan_cache[scan_id] = scan_result
        return scan_result

    def _parse_scan_output(self, output: str) -> List[Dict[str, Any]]:
        """Parse skillbox scan output into structured issues."""
        issues = []
        current_issue: Optional[Dict[str, Any]] = None

        for line in output.split('\n'):
            line = line.strip()
            if not line:
                continue

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
                    "code_snippet": ""
                }
            elif current_issue:
                if 'Rule:' in line:
                    current_issue["rule_id"] = line.split('Rule:')[-1].strip()
                elif 'Line' in line:
                    try:
                        line_num = int(line.split('Line')[-1].split(':')[0].strip())
                        current_issue["line_number"] = line_num
                    except ValueError:
                        pass
                elif 'Code:' in line or '│' in line:
                    current_issue["code_snippet"] = (
                        line.split('Code:')[-1].strip() if 'Code:' in line else line
                    )

        if current_issue:
            issues.append(current_issue)

        return issues

    # ==================== Confirmation Flow ====================

    def is_skill_confirmed(self, skill_name: str) -> bool:
        """Check if this skill has been confirmed recently."""
        if skill_name in self._confirmed_skills:
            confirmed_at = self._confirmed_skills[skill_name]
            if time.time() - confirmed_at < self._CONFIRMATION_TTL:
                return True
            del self._confirmed_skills[skill_name]
        return False

    def mark_skill_confirmed(self, skill_name: str) -> None:
        """Mark this skill as confirmed."""
        self._confirmed_skills[skill_name] = time.time()

    def handle_confirmation(
        self,
        skill_name: str,
        scan_result: SecurityScanResult,
    ) -> tuple[bool, Optional[str]]:
        """
        Handle the confirmation flow for a skill.

        Args:
            skill_name: Name of the skill
            scan_result: Security scan result

        Returns:
            Tuple of (should_proceed, error_message)
            - (True, None) if execution should proceed
            - (False, error_message) if execution should be cancelled
        """
        if not scan_result.requires_confirmation:
            return True, None

        if self.confirmation_callback:
            report = scan_result.format_report()
            confirmed = self.confirmation_callback(report, scan_result.scan_id)

            if confirmed:
                self.mark_skill_confirmed(skill_name)
                return True, None
            else:
                return False, f"🔐 Execution cancelled by user.\n\n{report}"
        else:
            return False, (
                f"🔐 Security Review Required\n\n"
                f"{scan_result.format_report()}\n\n"
                f"Provide a confirmation_callback when creating the adapter."
            )

    # ==================== Environment Management ====================

    def set_sandbox_level_env(self, level: str) -> Optional[str]:
        """Set sandbox level environment variable, return old value."""
        old_value = os.environ.get("SKILLBOX_SANDBOX_LEVEL")
        os.environ["SKILLBOX_SANDBOX_LEVEL"] = level
        return old_value

    def restore_sandbox_level_env(self, old_value: Optional[str]) -> None:
        """Restore sandbox level environment variable."""
        if old_value is not None:
            os.environ["SKILLBOX_SANDBOX_LEVEL"] = old_value
        elif "SKILLBOX_SANDBOX_LEVEL" in os.environ:
            del os.environ["SKILLBOX_SANDBOX_LEVEL"]


__all__ = ["BaseAdapter"]

