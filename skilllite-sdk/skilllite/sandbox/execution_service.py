"""
Unified Execution Service - High-level execution service for all entry points.

This module provides the UnifiedExecutionService class which is the single entry point
for all skill execution. It integrates security scanning, user confirmation, and
execution into a unified flow.

All entry points (AgenticLoop, LangChain, LlamaIndex, MCP) should use this service.

Key Features:
1. Unified security scanning
2. Unified confirmation flow
3. Unified execution
4. Context management
5. Temporary context overrides
"""

import hashlib
import os
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Callable, Dict, Optional, TYPE_CHECKING

from .base import ExecutionResult
from .context import ExecutionContext
from .unified_executor import UnifiedExecutor

if TYPE_CHECKING:
    from ..core.skill_info import SkillInfo


# Type alias for confirmation callback
ConfirmationCallback = Callable[[str, str], bool]


class UnifiedExecutionService:
    """
    Unified execution service - single entry point for all skill execution.

    This service integrates:
    1. Security scanning (via SecurityScanner)
    2. User confirmation flow
    3. Skill execution (via UnifiedExecutor)
    4. Session-level confirmation cache (avoid repeated prompts)

    Usage:
        service = UnifiedExecutionService()
        result = service.execute_skill(skill_info, input_data, confirmation_callback)
        
    For dependency injection:
        scanner = SecurityScanner()
        service = UnifiedExecutionService(scanner=scanner)
    """

    def __init__(self, scanner: Optional["SecurityScanner"] = None):
        """
        Initialize the service.
        
        Args:
            scanner: Optional SecurityScanner instance. If None, creates a new one lazily.
        """
        self._executor = UnifiedExecutor()
        self._scanner = scanner  # Can be injected or lazy initialized
        # Session-level confirmation cache: skill_name -> code_hash
        # When user confirms a skill, we cache it so the same skill
        # won't prompt again within the same session (unless code changes).
        self._confirmed_skills: Dict[str, str] = {}
    
    def _get_scanner(self):
        """Get security scanner (lazy initialization if not injected)."""
        if self._scanner is None:
            from ..core.security import SecurityScanner
            self._scanner = SecurityScanner()
        return self._scanner
    
    def execute_skill(
        self,
        skill_info: "SkillInfo",
        input_data: Dict[str, Any],
        entry_point: Optional[str] = None,
        confirmation_callback: Optional[ConfirmationCallback] = None,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
    ) -> ExecutionResult:
        """
        Execute a skill with unified security and confirmation flow.
        
        Flow:
        1. Read current execution context from environment
        2. Check if skill requires elevated permissions
        3. Perform security scan (if Level 3)
        4. Request user confirmation (if high-severity issues)
        5. Adjust context based on confirmation
        6. Execute skill
        
        Args:
            skill_info: SkillInfo object with skill metadata
            input_data: Input data for the skill
            entry_point: Optional specific script to execute
            confirmation_callback: Callback for security confirmation
            allow_network: Override network setting
            timeout: Override timeout setting
            
        Returns:
            ExecutionResult with output or error
        """
        # 1. Read current context from environment
        context = ExecutionContext.from_current_env()

        # 2. Apply overrides
        if allow_network is not None or timeout is not None:
            context = context.with_override(
                allow_network=allow_network,
                timeout=timeout,
            )

        # 3. Check if skill requires elevated permissions
        requires_elevated = self._requires_elevated_permissions(skill_info)
        if requires_elevated:
            context = context.with_elevated_permissions()
        
        # 4. Security scan and confirmation (Level 3 only)
        if context.sandbox_level == "3":
            skill_name = skill_info.name
            code_hash = self._compute_skill_code_hash(skill_info, entry_point)

            # Check session-level confirmation cache
            if (skill_name in self._confirmed_skills
                    and self._confirmed_skills[skill_name] == code_hash):
                # Already confirmed in this session, skip scan
                context = context.with_user_confirmation("")
            else:
                scan_result = self._perform_security_scan(
                    skill_info, input_data, entry_point
                )

                if scan_result and scan_result.requires_confirmation:
                    if confirmation_callback:
                        report = scan_result.format_report()
                        confirmed = confirmation_callback(report, scan_result.scan_id)

                        if confirmed:
                            # User confirmed -> downgrade (Level 1 or 2 from POST_CONFIRMATION_SANDBOX_LEVEL)
                            context = context.with_user_confirmation(scan_result.scan_id)
                            # Cache confirmation for this session
                            self._confirmed_skills[skill_name] = code_hash
                        else:
                            return ExecutionResult(
                                success=False,
                                error="Execution cancelled by user after security review",
                                exit_code=1,
                            )
                    else:
                        # No callback, return security report
                        return ExecutionResult(
                            success=False,
                            error=f"Security confirmation required:\n{scan_result.format_report()}",
                            exit_code=2,
                        )
                else:
                    # Scan passed (no high-severity issues) -> downgrade to Level 1 or 2.
                    # Use Level 2 when POST_CONFIRMATION_SANDBOX_LEVEL=2 for sandbox isolation.
                    post_level = os.environ.get("POST_CONFIRMATION_SANDBOX_LEVEL", "1").strip()
                    context = context.with_override(
                        sandbox_level="2" if post_level == "2" else "1"
                    )
        
        # 5. Execute skill
        return self._executor.execute(
            context=context,
            skill_dir=skill_info.path,
            input_data=input_data,
            entry_point=entry_point,
        )

    def execute_with_context(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        input_data: Dict[str, Any],
        entry_point: Optional[str] = None,
        args: Optional[list] = None,
    ) -> ExecutionResult:
        """
        Execute with explicit context (bypasses security scan).

        Use this when you've already performed security checks
        and have a prepared context.

        Args:
            context: Pre-configured execution context
            skill_dir: Path to skill directory
            input_data: Input data for the skill
            entry_point: Optional specific script to execute
            args: Optional command line arguments

        Returns:
            ExecutionResult with output or error
        """
        return self._executor.execute(
            context=context,
            skill_dir=skill_dir,
            input_data=input_data,
            entry_point=entry_point,
            args=args,
        )

    def _requires_elevated_permissions(self, skill_info: "SkillInfo") -> bool:
        """Check if skill requires elevated permissions."""
        if skill_info.metadata:
            return getattr(skill_info.metadata, 'requires_elevated_permissions', False)
        return False

    def _compute_skill_code_hash(
        self,
        skill_info: "SkillInfo",
        entry_point: Optional[str] = None,
    ) -> str:
        """Compute hash of skill's entry script for confirmation caching.

        The hash changes when the actual script code changes, so a re-confirmation
        is triggered if the skill is modified.
        """
        if entry_point:
            script = skill_info.path / entry_point
        elif skill_info.metadata and skill_info.metadata.entry_point:
            script = skill_info.path / skill_info.metadata.entry_point
        else:
            script = None
            for default in ["scripts/main.py", "main.py"]:
                candidate = skill_info.path / default
                if candidate.exists():
                    script = candidate
                    break

        if script and script.exists():
            try:
                content = script.read_bytes()
                return hashlib.sha256(content).hexdigest()[:16]
            except Exception:
                return ""
        return ""

    def _perform_security_scan(
        self,
        skill_info: "SkillInfo",
        input_data: Dict[str, Any],
        entry_point: Optional[str] = None,
    ):
        """Perform security scan on skill.

        Fail-secure: if scanning raises an unexpected error, return a result
        that requires confirmation rather than silently allowing execution.
        """
        try:
            scanner = self._get_scanner()
            return scanner.scan_skill(skill_info, input_data, entry_point=entry_point)
        except Exception:
            # Fail-secure: treat scan exceptions as requiring confirmation
            from ..core.security import SecurityScanResult
            return SecurityScanResult(
                is_safe=False,
                issues=[{
                    "severity": "High",
                    "issue_type": "Scan Error",
                    "rule_id": "scan-exception",
                    "line_number": 0,
                    "description": "Security scan encountered an unexpected error. "
                                   "Manual review required.",
                    "code_snippet": "",
                }],
                scan_id="error",
                code_hash="",
                high_severity_count=1,
            )

    @contextmanager
    def temporary_context(
        self,
        sandbox_level: Optional[str] = None,
        allow_network: Optional[bool] = None,
    ):
        """
        Context manager for temporary execution context changes.

        This temporarily modifies environment variables and restores
        them when the context exits.

        Usage:
            with service.temporary_context(sandbox_level="1"):
                result = service.execute_skill(...)

        Args:
            sandbox_level: Temporary sandbox level
            allow_network: Temporary network setting
        """
        old_sandbox_level = os.environ.get("SKILLBOX_SANDBOX_LEVEL")
        old_allow_network = os.environ.get("SKILLBOX_ALLOW_NETWORK")

        try:
            if sandbox_level is not None:
                os.environ["SKILLBOX_SANDBOX_LEVEL"] = sandbox_level
            if allow_network is not None:
                os.environ["SKILLBOX_ALLOW_NETWORK"] = "true" if allow_network else "false"
            yield
        finally:
            # Restore original values
            if sandbox_level is not None:
                if old_sandbox_level is not None:
                    os.environ["SKILLBOX_SANDBOX_LEVEL"] = old_sandbox_level
                elif "SKILLBOX_SANDBOX_LEVEL" in os.environ:
                    del os.environ["SKILLBOX_SANDBOX_LEVEL"]

            if allow_network is not None:
                if old_allow_network is not None:
                    os.environ["SKILLBOX_ALLOW_NETWORK"] = old_allow_network
                elif "SKILLBOX_ALLOW_NETWORK" in os.environ:
                    del os.environ["SKILLBOX_ALLOW_NETWORK"]

    def get_current_context(self) -> ExecutionContext:
        """Get current execution context from environment."""
        return ExecutionContext.from_current_env()


__all__ = ["UnifiedExecutionService", "ConfirmationCallback"]

