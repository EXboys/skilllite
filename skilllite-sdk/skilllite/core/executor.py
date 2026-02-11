"""
Skill executor - interfaces with the Rust skillbox binary.

This module provides a thin wrapper around UnifiedExecutionService for backward
compatibility. All execution goes through the unified path (security scan,
confirmation, execution).

This is a CORE module - do not modify without explicit permission.
"""

from pathlib import Path
from typing import Any, Dict, Optional

from ..sandbox.base import ExecutionResult
from ..sandbox.execution_service import UnifiedExecutionService

__all__ = ['SkillExecutor', 'ExecutionResult']


class SkillExecutor:
    """
    Executes skills using the skillbox binary.

    This class provides a Python interface to the Rust-based sandbox executor.
    Supports both traditional skill execution (via entry_point in SKILL.md) and
    direct script execution (via exec command).

    Delegates to UnifiedExecutionService for consistent security and execution flow.
    """

    def __init__(
        self,
        binary_path: Optional[str] = None,
        cache_dir: Optional[str] = None,
        allow_network: bool = False,
        enable_sandbox: bool = True,
        execution_timeout: Optional[int] = None,
        max_memory_mb: Optional[int] = None,
        sandbox_level: Optional[str] = None,
        auto_install: bool = False
    ):
        """
        Initialize the executor.

        Args:
            binary_path: Path to the skillbox binary. If None, auto-detect.
            cache_dir: Directory for caching virtual environments.
            allow_network: Whether to allow network access by default.
            enable_sandbox: Whether to enable sandbox protection (default: True).
            execution_timeout: Skill execution timeout in seconds (default: 120).
            max_memory_mb: Maximum memory limit in MB (default: 512).
            sandbox_level: Sandbox security level (1/2/3, default from env or 3).
            auto_install: Automatically download and install binary if not found.
        """
        self._binary_path = binary_path
        self._cache_dir = cache_dir
        self._allow_network = allow_network
        self._enable_sandbox = enable_sandbox
        self._execution_timeout = execution_timeout or 120
        self._max_memory_mb = max_memory_mb or 512
        self._sandbox_level = sandbox_level or "3"
        self._auto_install = auto_install
        self._service = UnifiedExecutionService()

    def _get_skill_info(self, skill_dir: Path) -> "SkillInfo":
        """Build SkillInfo from skill_dir for UnifiedExecutionService."""
        from .metadata import parse_skill_metadata
        from .skill_info import SkillInfo
        metadata = parse_skill_metadata(skill_dir)
        return SkillInfo(metadata=metadata, path=skill_dir)

    @property
    def binary_path(self) -> str:
        """Path to the skillbox binary."""
        from ..sandbox.skillbox import find_binary
        return self._binary_path or find_binary() or ""

    @property
    def cache_dir(self) -> Optional[str]:
        """Directory for caching virtual environments."""
        return self._cache_dir

    @property
    def allow_network(self) -> bool:
        """Whether network access is allowed by default."""
        return self._allow_network

    @property
    def enable_sandbox(self) -> bool:
        """Whether sandbox protection is enabled."""
        return self._enable_sandbox

    @property
    def execution_timeout(self) -> int:
        """Skill execution timeout in seconds."""
        return self._execution_timeout

    @property
    def max_memory_mb(self) -> int:
        """Maximum memory limit in MB."""
        return self._max_memory_mb

    @property
    def sandbox_level(self) -> str:
        """Sandbox security level (1/2/3)."""
        return self._sandbox_level

    @property
    def is_available(self) -> bool:
        """Check if skillbox is available and ready to use."""
        return bool(self.binary_path)

    @property
    def name(self) -> str:
        """Return the name of this sandbox implementation."""
        return "skillbox"

    def _build_context(self, allow_network: Optional[bool], timeout: Optional[int], enable_sandbox: Optional[bool]) -> "ExecutionContext":
        """Build ExecutionContext from instance config and overrides. Bypasses Python scan (preserves original SkillboxExecutor behavior)."""
        from ..sandbox.context import ExecutionContext
        use_sandbox = enable_sandbox if enable_sandbox is not None else self._enable_sandbox
        sandbox_level = "1" if not use_sandbox else self._sandbox_level
        return ExecutionContext.from_current_env().with_override(
            allow_network=allow_network if allow_network is not None else self._allow_network,
            timeout=timeout if timeout is not None else self._execution_timeout,
            sandbox_level=sandbox_level,
        )

    def execute(
        self,
        skill_dir: Path,
        input_data: Dict[str, Any],
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
        entry_point: Optional[str] = None,
        enable_sandbox: Optional[bool] = None
    ) -> ExecutionResult:
        """
        Execute a skill with the given input.

        Uses execute_with_context to preserve original behavior: no Python-layer
        security scan (security handled by skillbox binary). Same as SkillboxExecutor.

        Args:
            skill_dir: Path to the skill directory
            input_data: Input data for the skill
            allow_network: Override default network setting
            timeout: Execution timeout in seconds
            entry_point: Optional specific script to execute (e.g., "scripts/init_skill.py").
                        If provided, uses exec_script instead of run command.
            enable_sandbox: Override default sandbox setting

        Returns:
            ExecutionResult with the output or error
        """
        context = self._build_context(allow_network, timeout, enable_sandbox)
        skill_info = self._get_skill_info(skill_dir)
        if getattr(skill_info.metadata, "requires_elevated_permissions", False):
            context = context.with_elevated_permissions()
        return self._service.execute_with_context(
            context=context,
            skill_dir=skill_dir,
            input_data=input_data,
            entry_point=entry_point,
        )

    def exec_script(
        self,
        skill_dir: Path,
        script_path: str,
        input_data: Dict[str, Any],
        args: Optional[list] = None,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
        enable_sandbox: Optional[bool] = None
    ) -> ExecutionResult:
        """
        Execute a specific script directly.

        Uses execute_with_context to preserve original behavior: no Python-layer
        security scan, and args parameter is passed through.

        Args:
            skill_dir: Path to the skill directory
            script_path: Relative path to the script (e.g., "scripts/init_skill.py")
            input_data: Input data for the script. For CLI scripts using argparse,
                       this will be automatically converted to command line arguments.
            args: Optional command line arguments list (overrides auto-conversion)
            allow_network: Override default network setting
            timeout: Execution timeout in seconds
            enable_sandbox: Override default sandbox setting

        Returns:
            ExecutionResult with the output or error
        """
        context = self._build_context(allow_network, timeout, enable_sandbox)
        skill_info = self._get_skill_info(skill_dir)
        if getattr(skill_info.metadata, "requires_elevated_permissions", False):
            context = context.with_elevated_permissions()
        return self._service.execute_with_context(
            context=context,
            skill_dir=skill_dir,
            input_data=input_data,
            entry_point=script_path,
            args=args,
        )
