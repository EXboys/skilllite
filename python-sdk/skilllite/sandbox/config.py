"""
Sandbox configuration management.

SandboxConfig: Setup/init config (binary_path, cache_dir, enable_sandbox, auto_install).
ExecutionContext: Execution-time config (sandbox_level, allow_network, timeout, etc.).

Use config.to_context() to get ExecutionContext for running skills.
"""

import os
from dataclasses import dataclass, field
from typing import Optional, TYPE_CHECKING

from ..config.env_config import parse_bool_env

# Default configuration values (shared with ExecutionContext)
DEFAULT_EXECUTION_TIMEOUT = 120  # seconds
DEFAULT_MAX_MEMORY_MB = 512  # MB
DEFAULT_SANDBOX_LEVEL = "3"  # Level 3: Sandbox isolation + static code scanning
DEFAULT_ALLOW_NETWORK = False
DEFAULT_ENABLE_SANDBOX = True
DEFAULT_AUTO_INSTALL = False

if TYPE_CHECKING:
    from .context import ExecutionContext


@dataclass
class SandboxConfig:
    """
    Setup configuration for sandbox (binary path, cache, enable_sandbox, auto_install).

    Execution params (timeout, memory, allow_network, sandbox_level) live in
    ExecutionContext. Use to_context() to get execution config.

    Environment Variables:
        SKILLLITE_BINARY_PATH: Path to the skilllite binary
        SKILLBOX_CACHE_DIR: Directory for caching virtual environments
        SKILLBOX_ENABLE_SANDBOX: Enable sandbox protection (true/false)
        SKILLBOX_AUTO_INSTALL: Auto-install binary if not found (true/false)
    """
    binary_path: Optional[str] = None
    cache_dir: Optional[str] = None
    enable_sandbox: bool = field(default_factory=lambda: parse_bool_env("SKILLBOX_ENABLE_SANDBOX", DEFAULT_ENABLE_SANDBOX))
    auto_install: bool = field(default_factory=lambda: parse_bool_env("SKILLBOX_AUTO_INSTALL", DEFAULT_AUTO_INSTALL))

    def to_context(self) -> "ExecutionContext":
        """Build ExecutionContext from env. When enable_sandbox=False, uses sandbox_level=1."""
        from .context import ExecutionContext
        ctx = ExecutionContext.from_current_env()
        if not self.enable_sandbox:
            ctx = ctx.with_override(sandbox_level="1")
        return ctx

    @classmethod
    def from_env(cls) -> "SandboxConfig":
        """Create config from environment variables."""
        return cls(
            binary_path=os.environ.get("SKILLBOX_BINARY_PATH"),
            cache_dir=os.environ.get("SKILLBOX_CACHE_DIR"),
        )

    def with_overrides(
        self,
        binary_path: Optional[str] = None,
        cache_dir: Optional[str] = None,
        enable_sandbox: Optional[bool] = None,
        auto_install: Optional[bool] = None,
    ) -> "SandboxConfig":
        """Create a new config with specified overrides."""
        return SandboxConfig(
            binary_path=binary_path if binary_path is not None else self.binary_path,
            cache_dir=cache_dir if cache_dir is not None else self.cache_dir,
            enable_sandbox=enable_sandbox if enable_sandbox is not None else self.enable_sandbox,
            auto_install=auto_install if auto_install is not None else self.auto_install,
        )
