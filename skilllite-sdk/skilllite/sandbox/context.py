"""
Execution Context - Encapsulates all configuration for a single execution.

This module provides the ExecutionContext class which is the single source of truth
for execution configuration. It reads from environment variables at runtime,
ensuring that any changes to environment variables are immediately reflected.

Design Principles:
1. Read configuration at runtime, not at initialization
2. Never cache configuration values
3. Support temporary overrides via with_override()
4. Immutable - create new instances for modifications
"""

import os
from dataclasses import dataclass, field
from typing import Optional


# Default configuration values
DEFAULT_SANDBOX_LEVEL = "3"
DEFAULT_TIMEOUT = 120
DEFAULT_MAX_MEMORY_MB = 512
DEFAULT_ALLOW_NETWORK = False
DEFAULT_AUTO_APPROVE = False


def _parse_bool_env(key: str, default: bool) -> bool:
    """Parse a boolean value from environment variable."""
    value = os.environ.get(key)
    if value is None:
        return default
    value_lower = value.lower().strip()
    if value_lower in ("true", "1", "yes", "on"):
        return True
    elif value_lower in ("false", "0", "no", "off", ""):
        return False
    return default


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
    def from_current_env(cls) -> "ExecutionContext":
        """
        Create context from current environment variables.
        
        This method reads from environment variables at call time,
        ensuring the latest values are used.
        
        Environment Variables:
            SKILLBOX_SANDBOX_LEVEL: Sandbox level (1/2/3, default: 3)
            SKILLBOX_ALLOW_NETWORK: Allow network access (true/false)
            SKILLBOX_TIMEOUT_SECS: Execution timeout in seconds
            SKILLBOX_MAX_MEMORY_MB: Maximum memory in MB
            SKILLBOX_AUTO_APPROVE: Auto-approve security prompts
        """
        return cls(
            sandbox_level=os.environ.get("SKILLBOX_SANDBOX_LEVEL", DEFAULT_SANDBOX_LEVEL),
            allow_network=_parse_bool_env("SKILLBOX_ALLOW_NETWORK", DEFAULT_ALLOW_NETWORK),
            timeout=int(os.environ.get("SKILLBOX_TIMEOUT_SECS", str(DEFAULT_TIMEOUT))),
            max_memory_mb=int(os.environ.get("SKILLBOX_MAX_MEMORY_MB", str(DEFAULT_MAX_MEMORY_MB))),
            auto_approve=_parse_bool_env("SKILLBOX_AUTO_APPROVE", DEFAULT_AUTO_APPROVE),
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
        
        This downgrades sandbox level to 1 (no sandbox) since user has approved.
        """
        return self.with_override(
            sandbox_level="1",
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


__all__ = ["ExecutionContext", "DEFAULT_SANDBOX_LEVEL", "DEFAULT_TIMEOUT", "DEFAULT_MAX_MEMORY_MB"]

