"""
Skill executor - interfaces with the Rust skillbox binary.

Thin wrapper around ipc_executor (Phase 4.8 Strategy 7).
"""

from pathlib import Path
from typing import Any, Dict, Optional

from ..sandbox.base import ExecutionResult
from ..sandbox.ipc_executor import execute_with_context

__all__ = ['SkillExecutor', 'ExecutionResult']


class SkillExecutor:
    """
    Executes skills using the skillbox binary via ipc_executor.
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
        self._binary_path = binary_path
        self._cache_dir = cache_dir
        self._allow_network = allow_network
        self._enable_sandbox = enable_sandbox
        self._execution_timeout = execution_timeout or 120
        self._max_memory_mb = max_memory_mb or 512
        self._sandbox_level = sandbox_level or "3"
        self._auto_install = auto_install

    @property
    def binary_path(self) -> str:
        """Path to the skillbox binary."""
        from ..sandbox.core import find_binary
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
        return "skilllite"

    def _build_context(self, allow_network: Optional[bool], timeout: Optional[int], enable_sandbox: Optional[bool]) -> "ExecutionContext":
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
        context = self._build_context(allow_network, timeout, enable_sandbox)
        from .metadata import parse_skill_metadata
        meta = parse_skill_metadata(skill_dir)
        if getattr(meta, "requires_elevated_permissions", False):
            context = context.with_elevated_permissions()
        return execute_with_context(context, skill_dir, input_data, entry_point=entry_point)

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
        context = self._build_context(allow_network, timeout, enable_sandbox)
        from .metadata import parse_skill_metadata
        meta = parse_skill_metadata(skill_dir)
        if getattr(meta, "requires_elevated_permissions", False):
            context = context.with_elevated_permissions()
        return execute_with_context(
            context, skill_dir, input_data,
            entry_point=script_path,
            args=args,
        )
