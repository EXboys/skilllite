"""
Base classes for sandbox executors.

This module defines the abstract interface that all sandbox implementations
must follow, enabling easy switching between different sandbox backends.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Optional


@dataclass
class ExecutionResult:
    """Result of a sandbox execution."""
    success: bool
    output: Optional[Dict[str, Any]] = None
    error: Optional[str] = None
    exit_code: int = 0
    stdout: str = ""
    stderr: str = ""


class SandboxExecutor(ABC):
    """
    Abstract base class for sandbox executors.
    
    All sandbox implementations (skillbox, docker, pyodide, etc.) should
    inherit from this class and implement the required methods.
    """
    
    @abstractmethod
    def execute(
        self,
        skill_dir: Path,
        input_data: Dict[str, Any],
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None,
        entry_point: Optional[str] = None
    ) -> ExecutionResult:
        """
        Execute a skill with the given input.
        
        Args:
            skill_dir: Path to the skill directory
            input_data: Input data for the skill
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            entry_point: Optional specific script to execute
            
        Returns:
            ExecutionResult with the output or error
        """
        pass
    
    @abstractmethod
    def exec_script(
        self,
        skill_dir: Path,
        script_path: str,
        input_data: Dict[str, Any],
        args: Optional[list] = None,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> ExecutionResult:
        """
        Execute a specific script directly.
        
        Args:
            skill_dir: Path to the skill directory
            script_path: Relative path to the script
            input_data: Input data for the script
            args: Optional command line arguments
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            
        Returns:
            ExecutionResult with the output or error
        """
        pass
    
    @property
    @abstractmethod
    def is_available(self) -> bool:
        """Check if this sandbox executor is available and ready to use."""
        pass
    
    @property
    @abstractmethod
    def name(self) -> str:
        """Return the name of this sandbox implementation."""
        pass
