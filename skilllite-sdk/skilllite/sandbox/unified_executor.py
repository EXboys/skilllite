"""
Unified Executor - Single source of truth for all skill execution.

This module provides the UnifiedExecutor class which handles all execution logic.
It uses ExecutionContext to get configuration at runtime, ensuring that any
changes to environment variables or context overrides are immediately reflected.

Key Design Principles:
1. Never use instance variables for configuration
2. Always read from ExecutionContext at execution time
3. Single command building logic
4. Single subprocess execution logic
"""

import json
import os
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Optional

from .base import ExecutionResult
from .context import ExecutionContext


class UnifiedExecutor:
    """
    Unified executor - all skill execution goes through this class.
    
    This class is stateless regarding configuration. All configuration
    comes from ExecutionContext passed to each method.
    """
    
    def __init__(self):
        """Initialize the executor."""
        from .skillbox import find_binary
        self._binary_path = find_binary()
        if not self._binary_path:
            raise RuntimeError("skillbox binary not found")
    
    @property
    def binary_path(self) -> str:
        """Path to the skillbox binary."""
        return self._binary_path
    
    @property
    def is_available(self) -> bool:
        """Check if skillbox is available."""
        return self._binary_path is not None and os.path.exists(self._binary_path)
    
    def execute(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        input_data: Dict[str, Any],
        entry_point: Optional[str] = None,
        args: Optional[List[str]] = None,
    ) -> ExecutionResult:
        """
        Execute a skill with the given context.
        
        Args:
            context: Execution context with all configuration
            skill_dir: Path to the skill directory
            input_data: Input data for the skill
            entry_point: Optional specific script to execute
            args: Optional command line arguments
            
        Returns:
            ExecutionResult with output or error
        """
        if entry_point:
            return self.exec_script(
                context=context,
                skill_dir=skill_dir,
                script_path=entry_point,
                input_data=input_data,
                args=args,
            )
        
        # Build command for skillbox run
        cmd = self._build_run_command(context, skill_dir, input_data)
        return self._run_subprocess(cmd, context, skill_dir)
    
    def exec_script(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        script_path: str,
        input_data: Dict[str, Any],
        args: Optional[List[str]] = None,
    ) -> ExecutionResult:
        """
        Execute a specific script directly.
        
        Args:
            context: Execution context with all configuration
            skill_dir: Path to the skill directory
            script_path: Relative path to the script
            input_data: Input data for the script
            args: Optional command line arguments
            
        Returns:
            ExecutionResult with output or error
        """
        # Convert JSON input to CLI args if no explicit args provided
        if args is None and input_data:
            args = self._convert_json_to_cli_args(input_data)
        
        # For Level 1/2 Python scripts, use direct execution for better performance
        if script_path.endswith('.py') and context.sandbox_level != "3":
            return self._exec_python_direct(context, skill_dir, script_path, args)
        
        # Build command for skillbox exec
        cmd = self._build_exec_command(context, skill_dir, script_path, input_data, args)
        return self._run_subprocess(cmd, context, skill_dir)
    
    def _build_run_command(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        input_data: Dict[str, Any],
    ) -> List[str]:
        """Build command for skillbox run."""
        # Convert to absolute path to avoid path issues
        abs_skill_dir = Path(skill_dir).resolve()
        cmd = [
            self._binary_path,
            "run",
            str(abs_skill_dir),
            json.dumps(input_data),
        ]
        
        # Add sandbox level from context (NOT from instance variable)
        cmd.extend(["--sandbox-level", context.sandbox_level])
        
        if context.allow_network:
            cmd.append("--allow-network")
        
        cmd.extend(["--timeout", str(context.timeout)])
        cmd.extend(["--max-memory", str(context.max_memory_mb)])
        
        return cmd
    
    def _build_exec_command(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        script_path: str,
        input_data: Dict[str, Any],
        args: Optional[List[str]] = None,
    ) -> List[str]:
        """Build command for skillbox exec."""
        # Convert to absolute path to avoid path issues
        abs_skill_dir = Path(skill_dir).resolve()
        cmd = [
            self._binary_path,
            "exec",
            str(abs_skill_dir),
            script_path,
            json.dumps(input_data),
        ]
        
        if args:
            args_str = " ".join(args) if isinstance(args, list) else args
            cmd.extend(["--args", args_str])
        
        # Add sandbox level from context (NOT from instance variable)
        cmd.extend(["--sandbox-level", context.sandbox_level])
        
        if context.allow_network:
            cmd.append("--allow-network")
        
        cmd.extend(["--timeout", str(context.timeout)])
        cmd.extend(["--max-memory", str(context.max_memory_mb)])

        return cmd

    def _run_subprocess(
        self,
        cmd: List[str],
        context: ExecutionContext,
        skill_dir: Path,
    ) -> ExecutionResult:
        """Run subprocess with the given command."""
        env = self._build_env(context, skill_dir)

        try:
            if context.sandbox_level == "3" and not context.confirmed:
                # Level 3 without confirmation: allow stderr for prompts
                result = subprocess.run(
                    cmd,
                    stdin=None,
                    stdout=subprocess.PIPE,
                    stderr=None,
                    text=True,
                    timeout=context.timeout,
                    env=env,
                )
                return self._parse_output(result.stdout, "", result.returncode)
            else:
                # Level 1/2 or confirmed: capture all output
                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=context.timeout,
                    env=env,
                )
                return self._parse_output(result.stdout, result.stderr, result.returncode)

        except subprocess.TimeoutExpired:
            return ExecutionResult(
                success=False,
                error=f"Execution timed out after {context.timeout} seconds",
                exit_code=-1,
            )
        except FileNotFoundError:
            return ExecutionResult(
                success=False,
                error=f"skillbox binary not found at: {self._binary_path}",
                exit_code=-1,
            )
        except Exception as e:
            return ExecutionResult(
                success=False,
                error=f"Execution failed: {str(e)}",
                exit_code=-1,
            )

    def _build_env(
        self,
        context: ExecutionContext,
        skill_dir: Path,
    ) -> Dict[str, str]:
        """Build environment variables for subprocess."""
        env = os.environ.copy()

        # Set sandbox level in environment (for consistency)
        env["SKILLBOX_SANDBOX_LEVEL"] = context.sandbox_level
        env["SKILLBOX_AUTO_APPROVE"] = "1" if context.auto_approve or context.confirmed else "0"

        # Set skill-specific environment
        env["SKILL_DIR"] = str(skill_dir)
        env["SKILLBOX_TIMEOUT_SECS"] = str(context.timeout)
        env["SKILLBOX_MAX_MEMORY_MB"] = str(context.max_memory_mb)

        return env

    def _ensure_skill_python(self, skill_dir: Path) -> str:
        """Get Python executable with dependencies installed if needed.

        If the skill has dependencies (from ``.skilllite.lock`` or the
        ``compatibility`` field in SKILL.md), ensures a virtual environment
        exists with those deps installed and returns the venv's python path.
        Otherwise returns ``sys.executable``.

        This mirrors what the Rust ``ensure_environment()`` does for Level 3,
        so that Level 1/2 direct execution also gets automatic dependency
        management without requiring ``skilllite init``.
        """
        import sys

        try:
            from ..core.metadata import parse_skill_metadata
            from ..cli.init import (
                parse_compatibility_for_packages,
                _get_cache_dir,
                _compute_packages_hash,
                _get_cache_key,
                _ensure_python_env,
            )
        except ImportError:
            return sys.executable

        try:
            metadata = parse_skill_metadata(skill_dir)
        except Exception:
            return sys.executable

        # Prefer resolved_packages from .skilllite.lock, fallback to whitelist parsing
        packages = metadata.resolved_packages
        if packages is None:
            packages = parse_compatibility_for_packages(
                metadata.compatibility
            )

        if not packages:
            return sys.executable

        # Compute cache key and ensure venv exists
        language = metadata.language or "python"
        content_hash = _compute_packages_hash(packages)
        cache_key = _get_cache_key(language, content_hash)
        cache_dir = _get_cache_dir()
        cache_dir.mkdir(parents=True, exist_ok=True)
        env_path = cache_dir / cache_key

        # Create venv and install packages (idempotent — skips if marker exists)
        _ensure_python_env(env_path, packages)

        # Return venv's python executable
        python = (
            env_path / "Scripts" / "python"
            if os.name == "nt"
            else env_path / "bin" / "python"
        )
        return str(python) if python.exists() else sys.executable

    def _exec_python_direct(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        script_path: str,
        args: Optional[List[str]] = None,
    ) -> ExecutionResult:
        """Execute Python script directly (for Level 1/2)."""
        import sys

        # Convert to absolute path to avoid path duplication issues
        abs_skill_dir = Path(skill_dir).resolve()
        full_script_path = abs_skill_dir / script_path

        if not full_script_path.exists():
            return ExecutionResult(
                success=False,
                error=f"Script not found: {full_script_path}",
                exit_code=-1,
            )

        # Ensure dependencies are installed and get the correct python executable
        try:
            python_executable = self._ensure_skill_python(abs_skill_dir)
        except Exception as e:
            return ExecutionResult(
                success=False,
                error=f"Failed to install skill dependencies: {e}",
                exit_code=-1,
            )

        cmd = [python_executable, str(full_script_path)]
        if args:
            cmd.extend(args)

        env = self._build_env(context, abs_skill_dir)
        env["PYTHONPATH"] = str(abs_skill_dir)

        try:
            # Don't set cwd to skill_dir - let scripts run from project root
            # Scripts can use SKILL_DIR env var to find their own location
            # This allows scripts like skill-creator to work with relative paths
            # that are relative to the project root, not the skill directory
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=context.timeout,
                env=env,
            )
            return self._parse_output(result.stdout, result.stderr, result.returncode)
        except subprocess.TimeoutExpired:
            return ExecutionResult(
                success=False,
                error=f"Execution timed out after {context.timeout} seconds",
                exit_code=-1,
            )
        except Exception as e:
            return ExecutionResult(
                success=False,
                error=f"Execution failed: {str(e)}",
                exit_code=-1,
            )

    def _convert_json_to_cli_args(self, input_data: Dict[str, Any]) -> List[str]:
        """Convert JSON input to CLI arguments.

        Delegates to the shared utility function that properly handles
        positional arguments like 'skill_name'.
        """
        from .utils import convert_json_to_cli_args
        return convert_json_to_cli_args(input_data)

    def _parse_output(
        self,
        stdout: str,
        stderr: str,
        returncode: int,
    ) -> ExecutionResult:
        """Parse subprocess output into ExecutionResult."""
        combined = stdout + stderr

        # Try to parse JSON output
        try:
            # Look for JSON in output
            for line in combined.split('\n'):
                line = line.strip()
                if line.startswith('{') and line.endswith('}'):
                    data = json.loads(line)
                    if isinstance(data, dict):
                        return ExecutionResult(
                            success=returncode == 0,
                            output=data,
                            exit_code=returncode,
                            stdout=stdout,
                            stderr=stderr,
                        )
        except json.JSONDecodeError:
            pass

        # Return as plain text
        if returncode == 0:
            return ExecutionResult(
                success=True,
                output={"result": stdout.strip()} if stdout.strip() else None,
                exit_code=returncode,
                stdout=stdout,
                stderr=stderr,
            )
        else:
            error_msg = stderr.strip() if stderr.strip() else stdout.strip()
            return ExecutionResult(
                success=False,
                error=f"Skill execution failed with exit code {returncode}: {error_msg}",
                exit_code=returncode,
                stdout=stdout,
                stderr=stderr,
            )


__all__ = ["UnifiedExecutor"]
