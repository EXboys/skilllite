"""
Skill executor - interfaces with the Rust skillbox binary.

This module provides a thin wrapper around SkillboxExecutor for backward compatibility.
All actual execution logic is delegated to the sandbox.skillbox.executor module.

This is a CORE module - do not modify without explicit permission.
"""

from pathlib import Path
from typing import Any, Dict, Optional

from ..sandbox.base import ExecutionResult
from ..sandbox.skillbox.executor import SkillboxExecutor

__all__ = ['SkillExecutor', 'ExecutionResult']


class SkillExecutor:
    """
    Executes skills using the skillbox binary.
    
    This class provides a Python interface to the Rust-based sandbox executor.
    Supports both traditional skill execution (via entry_point in SKILL.md) and
    direct script execution (via exec command).
    
    This is a thin wrapper around SkillboxExecutor for backward compatibility.
    All execution logic is delegated to SkillboxExecutor.
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
        self._executor = SkillboxExecutor(
            binary_path=binary_path,
            cache_dir=cache_dir,
            allow_network=allow_network,
            enable_sandbox=enable_sandbox,
            execution_timeout=execution_timeout,
            max_memory_mb=max_memory_mb,
            sandbox_level=sandbox_level,
            auto_install=auto_install
        )
    
    @property
    def binary_path(self) -> str:
        """Path to the skillbox binary."""
        return self._executor.binary_path
    
    @property
    def cache_dir(self) -> Optional[str]:
        """Directory for caching virtual environments."""
        return self._executor.cache_dir
    
    @property
    def allow_network(self) -> bool:
        """Whether network access is allowed by default."""
        return self._executor.allow_network
    
    @property
    def enable_sandbox(self) -> bool:
        """Whether sandbox protection is enabled."""
        return self._executor.enable_sandbox
    
    @property
    def execution_timeout(self) -> int:
        """Skill execution timeout in seconds."""
        return self._executor.execution_timeout
    
    @property
    def max_memory_mb(self) -> int:
        """Maximum memory limit in MB."""
        return self._executor.max_memory_mb
    
    @property
    def sandbox_level(self) -> str:
        """Sandbox security level (1/2/3)."""
        return self._executor.sandbox_level
    
    @property
    def is_available(self) -> bool:
        """Check if skillbox is available and ready to use."""
        return self._executor.is_available
    
    @property
    def name(self) -> str:
        """Return the name of this sandbox implementation."""
        return self._executor.name
    
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
        return self._executor.execute(
            skill_dir=skill_dir,
            input_data=input_data,
            allow_network=allow_network,
            timeout=timeout,
            entry_point=entry_point,
            enable_sandbox=enable_sandbox
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
        
        This method allows executing any script in the skill directory without
        requiring an entry_point in SKILL.md. Useful for skills with multiple
        scripts or prompt-only skills with helper scripts.
        
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
        return self._executor.exec_script(
            skill_dir=skill_dir,
            script_path=script_path,
            input_data=input_data,
            args=args,
            allow_network=allow_network,
            timeout=timeout,
            enable_sandbox=enable_sandbox
        )
