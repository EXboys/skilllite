"""
Execution Context - Encapsulates all configuration for a single execution.

Also includes: ExecutionResult, SandboxConfig, SandboxExecutor (from base/config merge).
"""

import os
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Optional

from ..config import parse_bool_env, get_timeout_from_env, get_memory_from_env

# Default values (single source of truth)
DEFAULT_EXECUTION_TIMEOUT = 120
DEFAULT_MAX_MEMORY_MB = 512
DEFAULT_SANDBOX_LEVEL = "3"
DEFAULT_ALLOW_NETWORK = False
DEFAULT_ENABLE_SANDBOX = True
DEFAULT_AUTO_INSTALL = False
DEFAULT_TIMEOUT = DEFAULT_EXECUTION_TIMEOUT
DEFAULT_AUTO_APPROVE = False


@dataclass
class ExecutionResult:
    """Result of a sandbox execution."""
    success: bool
    output: Optional[Dict[str, Any]] = None
    error: Optional[str] = None
    exit_code: int = 0
    stdout: str = ""
    stderr: str = ""


@dataclass
class SandboxConfig:
    """Setup configuration for sandbox (binary path, cache, enable_sandbox, auto_install)."""
    binary_path: Optional[str] = None
    cache_dir: Optional[str] = None
    enable_sandbox: bool = field(default_factory=lambda: parse_bool_env("SKILLBOX_ENABLE_SANDBOX", DEFAULT_ENABLE_SANDBOX))
    auto_install: bool = field(default_factory=lambda: parse_bool_env("SKILLBOX_AUTO_INSTALL", DEFAULT_AUTO_INSTALL))

    def to_context(self) -> "ExecutionContext":
        ctx = ExecutionContext.from_current_env()
        if not self.enable_sandbox:
            ctx = ctx.with_override(sandbox_level="1")
        return ctx

    @classmethod
    def from_env(cls) -> "SandboxConfig":
        return cls(
            binary_path=os.environ.get("SKILLBOX_BINARY_PATH"),
            cache_dir=os.environ.get("SKILLBOX_CACHE_DIR"),
        )

    def with_overrides(self, binary_path=None, cache_dir=None, enable_sandbox=None, auto_install=None) -> "SandboxConfig":
        return SandboxConfig(
            binary_path=binary_path if binary_path is not None else self.binary_path,
            cache_dir=cache_dir if cache_dir is not None else self.cache_dir,
            enable_sandbox=enable_sandbox if enable_sandbox is not None else self.enable_sandbox,
            auto_install=auto_install if auto_install is not None else self.auto_install,
        )


class SandboxExecutor(ABC):
    """Abstract base class for sandbox implementations."""
    @abstractmethod
    def execute(self, skill_dir: Path, input_data: Dict[str, Any], allow_network=None, timeout=None, entry_point=None) -> ExecutionResult:
        pass
    @abstractmethod
    def exec_script(self, skill_dir: Path, script_path: str, input_data: Dict[str, Any], args=None, allow_network=None, timeout=None) -> ExecutionResult:
        pass
    @property
    @abstractmethod
    def is_available(self) -> bool:
        pass
    @property
    @abstractmethod
    def name(self) -> str:
        pass


@dataclass(frozen=True)
class ExecutionContext:
    """
    Execution context - all configuration for a single execution.
    
    This class is immutable (frozen=True). To modify, use with_override()
    which returns a new instance.
    
    Attributes:
        sandbox_level: Sandbox security level ("1", "2", or "3")
        allow_network: Whether to allow network access
        timeout: Execution timeout in seconds
        max_memory_mb: Maximum memory limit in MB
        auto_approve: Whether to auto-approve security prompts
        confirmed: Whether user has confirmed execution (for security flow)
        scan_id: Scan ID from security scan (for verification)
        requires_elevated: Whether skill requires elevated permissions
    """
    sandbox_level: str = DEFAULT_SANDBOX_LEVEL
    allow_network: bool = DEFAULT_ALLOW_NETWORK
    timeout: int = DEFAULT_TIMEOUT
    max_memory_mb: int = DEFAULT_MAX_MEMORY_MB
    auto_approve: bool = DEFAULT_AUTO_APPROVE
    confirmed: bool = False
    scan_id: Optional[str] = None
    requires_elevated: bool = False
    
    @classmethod
    def from_config(cls, config: SandboxConfig) -> "ExecutionContext":
        """Build context from SandboxConfig. Applies enable_sandbox (False -> level 1)."""
        return config.to_context()

    @classmethod
    def from_current_env(cls) -> "ExecutionContext":
        """
        Create context from current environment variables.

        Environment Variables:
            SKILLBOX_SANDBOX_LEVEL, SKILLBOX_ALLOW_NETWORK,
            SKILLBOX_TIMEOUT_SECS, SKILLBOX_MAX_MEMORY_MB, SKILLBOX_AUTO_APPROVE
        """
        return cls(
            sandbox_level=os.environ.get("SKILLBOX_SANDBOX_LEVEL", DEFAULT_SANDBOX_LEVEL),
            allow_network=parse_bool_env("SKILLBOX_ALLOW_NETWORK", DEFAULT_ALLOW_NETWORK),
            timeout=get_timeout_from_env(),
            max_memory_mb=get_memory_from_env(),
            auto_approve=parse_bool_env("SKILLBOX_AUTO_APPROVE", DEFAULT_AUTO_APPROVE),
        )
    
    def with_override(
        self,
        sandbox_level: Optional[str] = None,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
        max_memory_mb: Optional[int] = None,
        auto_approve: Optional[bool] = None,
        confirmed: bool = False,
        scan_id: Optional[str] = None,
        requires_elevated: Optional[bool] = None,
    ) -> "ExecutionContext":
        """
        Create a new context with specified overrides.
        
        Args:
            sandbox_level: Override sandbox level
            allow_network: Override network setting
            timeout: Override timeout
            max_memory_mb: Override memory limit
            auto_approve: Override auto-approve setting
            confirmed: Set confirmed flag
            scan_id: Set scan ID
            requires_elevated: Set requires elevated flag
            
        Returns:
            New ExecutionContext with overrides applied
        """
        return ExecutionContext(
            sandbox_level=sandbox_level if sandbox_level is not None else self.sandbox_level,
            allow_network=allow_network if allow_network is not None else self.allow_network,
            timeout=timeout if timeout is not None else self.timeout,
            max_memory_mb=max_memory_mb if max_memory_mb is not None else self.max_memory_mb,
            auto_approve=auto_approve if auto_approve is not None else self.auto_approve,
            confirmed=confirmed if confirmed else self.confirmed,
            scan_id=scan_id if scan_id is not None else self.scan_id,
            requires_elevated=requires_elevated if requires_elevated is not None else self.requires_elevated,
        )
    
    def with_user_confirmation(self, scan_id: str) -> "ExecutionContext":
        """
        Create a new context after user confirmation.
        
        Confirm = grant permissions: keep L2 sandbox + relaxed (.env, git, venv/cache).
        If L2 is still insufficient, user can set SKILLBOX_SANDBOX_LEVEL=1.
        """
        return self.with_override(
            sandbox_level="2",
            confirmed=True,
            scan_id=scan_id,
        )
    
    def with_elevated_permissions(self) -> "ExecutionContext":
        """
        Create a new context with elevated permissions.
        
        This downgrades sandbox level to 1 for skills that require
        elevated permissions (e.g., skill-creator).
        """
        return self.with_override(
            sandbox_level="1",
            requires_elevated=True,
        )


__all__ = [
    "ExecutionContext",
    "ExecutionResult",
    "SandboxConfig",
    "SandboxExecutor",
    "DEFAULT_SANDBOX_LEVEL",
    "DEFAULT_TIMEOUT",
    "DEFAULT_MAX_MEMORY_MB",
    "DEFAULT_ALLOW_NETWORK",
    "DEFAULT_AUTO_APPROVE",
]

