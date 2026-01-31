"""
Sandbox configuration management.

This module provides centralized configuration for the sandbox executor,
including default values, environment variable handling, and validation.
"""

import os
from dataclasses import dataclass, field
from typing import Optional


# Default configuration values
DEFAULT_EXECUTION_TIMEOUT = 120  # seconds
DEFAULT_MAX_MEMORY_MB = 512  # MB
DEFAULT_SANDBOX_LEVEL = "3"  # Level 3: Sandbox isolation + static code scanning
DEFAULT_ALLOW_NETWORK = False
DEFAULT_ENABLE_SANDBOX = True
DEFAULT_AUTO_INSTALL = False


@dataclass
class SandboxConfig:
    """
    Configuration for sandbox execution.
    
    This class centralizes all configuration options for the sandbox executor,
    supporting both programmatic configuration and environment variables.
    
    Priority order (highest to lowest):
    1. Explicit constructor arguments
    2. Environment variables
    3. Default values
    
    Environment Variables:
        SKILLBOX_BINARY_PATH: Path to the skillbox binary
        SKILLBOX_CACHE_DIR: Directory for caching virtual environments
        SKILLBOX_SANDBOX_LEVEL: Security level (1/2/3)
        SKILLBOX_MAX_MEMORY_MB: Maximum memory limit in MB
        SKILLBOX_TIMEOUT_SECS: Execution timeout in seconds
        SKILLBOX_ALLOW_NETWORK: Allow network access (true/false/1/0)
        SKILLBOX_ENABLE_SANDBOX: Enable sandbox protection (true/false/1/0)
        SKILLBOX_AUTO_APPROVE: Auto-approve security prompts (true/false/1/0)
        
        # Legacy environment variables (deprecated, use SKILLBOX_* prefix)
        EXECUTION_TIMEOUT: Execution timeout in seconds
        MAX_MEMORY_MB: Maximum memory limit in MB
    
    Attributes:
        binary_path: Path to the skillbox binary. If None, auto-detect.
        cache_dir: Directory for caching virtual environments.
        allow_network: Whether to allow network access by default.
        enable_sandbox: Whether to enable sandbox protection.
        execution_timeout: Skill execution timeout in seconds.
        max_memory_mb: Maximum memory limit in MB.
        sandbox_level: Sandbox security level (1/2/3).
        auto_install: Automatically download and install binary if not found.
        auto_approve: Auto-approve security prompts in Level 3.
    """
    
    binary_path: Optional[str] = None
    cache_dir: Optional[str] = None
    allow_network: bool = field(default_factory=lambda: _parse_bool_env("SKILLBOX_ALLOW_NETWORK", DEFAULT_ALLOW_NETWORK))
    enable_sandbox: bool = field(default_factory=lambda: _parse_bool_env("SKILLBOX_ENABLE_SANDBOX", DEFAULT_ENABLE_SANDBOX))
    execution_timeout: int = field(default_factory=lambda: _get_timeout_from_env())
    max_memory_mb: int = field(default_factory=lambda: _get_memory_from_env())
    sandbox_level: str = field(default_factory=lambda: os.environ.get("SKILLBOX_SANDBOX_LEVEL", DEFAULT_SANDBOX_LEVEL))
    auto_install: bool = field(default_factory=lambda: _parse_bool_env("SKILLBOX_AUTO_INSTALL", DEFAULT_AUTO_INSTALL))
    auto_approve: bool = field(default_factory=lambda: _parse_bool_env("SKILLBOX_AUTO_APPROVE", False))
    
    def __post_init__(self):
        """Validate configuration after initialization."""
        self._validate()
    
    def _validate(self) -> None:
        """Validate configuration values."""
        # Validate sandbox level
        if self.sandbox_level not in ("1", "2", "3"):
            raise ValueError(
                f"Invalid sandbox_level '{self.sandbox_level}'. "
                f"Must be '1', '2', or '3'."
            )
        
        # Validate timeout
        if self.execution_timeout <= 0:
            raise ValueError(
                f"Invalid execution_timeout {self.execution_timeout}. "
                f"Must be a positive integer."
            )
        
        # Validate memory limit
        if self.max_memory_mb <= 0:
            raise ValueError(
                f"Invalid max_memory_mb {self.max_memory_mb}. "
                f"Must be a positive integer."
            )
    
    @classmethod
    def from_env(cls) -> "SandboxConfig":
        """
        Create configuration from environment variables only.
        
        Returns:
            SandboxConfig with values from environment variables.
        """
        return cls(
            binary_path=os.environ.get("SKILLBOX_BINARY_PATH"),
            cache_dir=os.environ.get("SKILLBOX_CACHE_DIR"),
        )
    
    def with_overrides(
        self,
        binary_path: Optional[str] = None,
        cache_dir: Optional[str] = None,
        allow_network: Optional[bool] = None,
        enable_sandbox: Optional[bool] = None,
        execution_timeout: Optional[int] = None,
        max_memory_mb: Optional[int] = None,
        sandbox_level: Optional[str] = None,
        auto_install: Optional[bool] = None,
    ) -> "SandboxConfig":
        """
        Create a new config with specified overrides.
        
        Args:
            binary_path: Override binary path
            cache_dir: Override cache directory
            allow_network: Override network setting
            enable_sandbox: Override sandbox setting
            execution_timeout: Override timeout
            max_memory_mb: Override memory limit
            sandbox_level: Override sandbox level
            auto_install: Override auto-install setting
            
        Returns:
            New SandboxConfig with overrides applied.
        """
        return SandboxConfig(
            binary_path=binary_path if binary_path is not None else self.binary_path,
            cache_dir=cache_dir if cache_dir is not None else self.cache_dir,
            allow_network=allow_network if allow_network is not None else self.allow_network,
            enable_sandbox=enable_sandbox if enable_sandbox is not None else self.enable_sandbox,
            execution_timeout=execution_timeout if execution_timeout is not None else self.execution_timeout,
            max_memory_mb=max_memory_mb if max_memory_mb is not None else self.max_memory_mb,
            sandbox_level=sandbox_level if sandbox_level is not None else self.sandbox_level,
            auto_install=auto_install if auto_install is not None else self.auto_install,
        )


def _parse_bool_env(key: str, default: bool) -> bool:
    """
    Parse a boolean value from environment variable.
    
    Accepts: true, false, 1, 0, yes, no (case-insensitive)
    
    Args:
        key: Environment variable name
        default: Default value if not set
        
    Returns:
        Parsed boolean value
    """
    value = os.environ.get(key)
    if value is None:
        return default
    
    value_lower = value.lower().strip()
    if value_lower in ("true", "1", "yes", "on"):
        return True
    elif value_lower in ("false", "0", "no", "off", ""):
        return False
    else:
        return default


def _get_timeout_from_env() -> int:
    """
    Get execution timeout from environment variables.
    
    Checks SKILLBOX_TIMEOUT_SECS first, then falls back to legacy EXECUTION_TIMEOUT.
    
    Returns:
        Timeout in seconds
    """
    # New environment variable (preferred)
    value = os.environ.get("SKILLBOX_TIMEOUT_SECS")
    if value:
        try:
            return int(value)
        except ValueError:
            pass
    
    # Legacy environment variable (deprecated)
    value = os.environ.get("EXECUTION_TIMEOUT")
    if value:
        try:
            return int(value)
        except ValueError:
            pass
    
    return DEFAULT_EXECUTION_TIMEOUT


def _get_memory_from_env() -> int:
    """
    Get memory limit from environment variables.
    
    Checks SKILLBOX_MAX_MEMORY_MB first, then falls back to legacy MAX_MEMORY_MB.
    
    Returns:
        Memory limit in MB
    """
    # New environment variable (preferred)
    value = os.environ.get("SKILLBOX_MAX_MEMORY_MB")
    if value:
        try:
            return int(value)
        except ValueError:
            pass
    
    # Legacy environment variable (deprecated)
    value = os.environ.get("MAX_MEMORY_MB")
    if value:
        try:
            return int(value)
        except ValueError:
            pass
    
    return DEFAULT_MAX_MEMORY_MB
