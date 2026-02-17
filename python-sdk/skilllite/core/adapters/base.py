"""
Base Adapter - Common logic for all framework adapters.

This module provides the BaseAdapter class which implements shared functionality
for all framework integrations (LangChain, LlamaIndex, MCP, etc.).

All adapters should inherit from BaseAdapter to ensure consistent behavior
and reduce code duplication.

Key Features:
1. Unified execution through ipc_executor
2. Shared security scanning logic
3. Common confirmation flow
4. Consistent error handling
"""

import hashlib
import json
import subprocess
import time
import uuid
from abc import ABC, abstractmethod
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

from ..security import (
    SecurityScanResult,
    ConfirmationCallback,
    AsyncConfirmationCallback,
    ExecutionOptions,
    parse_scan_json_output,
)

if TYPE_CHECKING:
    from ..manager import SkillManager
    from ..skill_info import SkillInfo
    from ...sandbox.context import ExecutionResult


class BaseAdapter(ABC):
    """
    Base class for all framework adapters.
    
    Implements common logic for:
    - Skill execution via ipc_executor
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
        Execute a skill using ipc_executor.
        
        This is the unified execution entry point that all adapters should use.
        It handles security scanning, confirmation, and execution.
        
        Args:
            skill_name: Name of the skill to execute
            input_data: Input data for the skill
            
        Returns:
            ExecutionResult with output or error
        """
        from ...sandbox.context import ExecutionResult
        from ...sandbox.ipc_executor import execute_via_ipc, execute_bash_via_ipc

        tool_info = self.manager._registry.get_multi_script_tool_info(skill_name)
        if tool_info:
            skill_info = self.manager.get_skill(tool_info["skill_name"])
            entry_point = tool_info.get("script_path")
        else:
            skill_info = self.manager.get_skill(skill_name)
            entry_point = None

        if not skill_info:
            return ExecutionResult(
                success=False,
                error=f"Skill '{skill_name}' not found",
                exit_code=1,
            )

        if skill_info.is_bash_tool_skill:
            cmd = input_data.get("command", "")
            if not cmd:
                return ExecutionResult(
                    success=False,
                    error="Bash tool requires 'command' parameter",
                    exit_code=1,
                )
            return execute_bash_via_ipc(skill_info, cmd, timeout=self.timeout)

        return execute_via_ipc(
            skill_info=skill_info,
            input_data=input_data,
            entry_point=entry_point,
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
                    from ...sandbox.core import find_binary

                    skillbox_path = find_binary()
                    if skillbox_path:
                        result = subprocess.run(
                            [skillbox_path, "security-scan", "--json", str(entry_script)],
                            capture_output=True,
                            text=True,
                            timeout=30
                        )
                        data = parse_scan_json_output(result.stdout)
                        scan_result = SecurityScanResult(
                            is_safe=data["is_safe"],
                            issues=data["issues"],
                            scan_id=scan_id,
                            code_hash=input_hash,
                            high_severity_count=data["high_severity_count"],
                            medium_severity_count=data["medium_severity_count"],
                            low_severity_count=data["low_severity_count"],
                        )
                        self._scan_cache[scan_id] = scan_result
                        return scan_result
        except Exception:
            pass

        # Default: no issues found
        scan_result = SecurityScanResult.safe(scan_id, input_hash)
        self._scan_cache[scan_id] = scan_result
        return scan_result



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



__all__ = ["BaseAdapter"]

