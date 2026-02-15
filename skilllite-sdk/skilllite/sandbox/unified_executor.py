"""
Unified Executor - Single source of truth for all skill execution.

This module provides the UnifiedExecutor class which handles all execution logic.
It uses ExecutionContext to get configuration at runtime, ensuring that any
changes to environment variables or context overrides are immediately reflected.

Supports both IPC (skillbox serve --stdio) and subprocess modes.
Set SKILLBOX_USE_IPC=0 to force subprocess.
"""

import json
import os
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Optional

from .base import ExecutionResult
from .context import ExecutionContext
from .utils import extract_json_from_output, format_sandbox_error


def _use_ipc() -> bool:
    """Check if IPC mode is enabled. Default: True. Set SKILLBOX_USE_IPC=0 to disable."""
    val = os.environ.get("SKILLBOX_USE_IPC", "").lower()
    return val not in ("0", "false", "no")


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
        self._ipc_client = None  # Lazy init when IPC is used
    
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
        
        if _use_ipc():
            result = self._execute_via_ipc_run(context, skill_dir, input_data)
            if result is not None:
                return result
        cmd, stdin_data = self._build_run_command(context, skill_dir, input_data)
        return self._run_subprocess(cmd, context, skill_dir, stdin_data=stdin_data)
    
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
        
        # All levels (1, 2, 3) go through skillbox for:
        # - Consistent dependency resolution (ensure_environment reads compatibility/lock)
        # - Level 2: proper sandbox isolation
        # - Level 1: resource limits and env_path with deps (no isolation)
        
        if _use_ipc():
            result = self._execute_via_ipc_exec(context, skill_dir, script_path, input_data, args)
            if result is not None:
                return result
        cmd = self._build_exec_command(context, skill_dir, script_path, input_data, args)
        return self._run_subprocess(cmd, context, skill_dir)
    
    # ==================== Bash Tool Skill Execution ====================

    def execute_bash(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        command: str,
    ) -> ExecutionResult:
        """Execute a bash command for a bash-tool skill via ``skillbox bash``.

        Security validation is handled entirely by the Rust binary. This method
        only routes the command to ``skillbox bash`` via IPC or subprocess.

        Args:
            context: Execution context with configuration.
            skill_dir: Path to the skill directory.
            command: The bash command string.

        Returns:
            ExecutionResult with stdout/stderr.
        """
        if _use_ipc():
            result = self._execute_via_ipc_bash(context, skill_dir, command)
            if result is not None:
                return result

        cmd = self._build_bash_command(context, skill_dir, command)
        return self._run_subprocess(cmd, context, skill_dir)

    def _build_bash_command(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        command: str,
    ) -> list:
        """Build command for ``skillbox bash``."""
        abs_skill_dir = Path(skill_dir).resolve()
        cmd = [
            self._binary_path,
            "bash",
            str(abs_skill_dir),
            command,
        ]
        cmd.extend(["--timeout", str(context.timeout or 120)])
        # Pass working directory so output files are saved relative to the user's workspace
        cmd.extend(["--cwd", os.getcwd()])
        return cmd

    def _execute_via_ipc_bash(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        command: str,
    ) -> Optional[ExecutionResult]:
        """Execute bash via IPC. Returns None on failure to fall back to subprocess."""
        try:
            pool = self._get_ipc_client()
            client = pool._get_client(timeout=context.timeout or 120)
            try:
                result = client._send_request(
                    method="bash",
                    params={
                        "skill_dir": str(Path(skill_dir).resolve()),
                        "command": command,
                        "timeout": context.timeout or 120,
                        "cwd": os.getcwd(),
                    },
                )
            finally:
                pool._return_client(client)

            # skillbox bash returns JSON string; parse it
            if isinstance(result, str):
                try:
                    result = json.loads(result)
                except (json.JSONDecodeError, TypeError):
                    return self._parse_output(result, "", 0)

            if isinstance(result, dict):
                stdout = result.get("stdout", "")
                stderr = result.get("stderr", "")
                exit_code = result.get("exit_code", 0)
                if exit_code == 0:
                    return ExecutionResult(
                        success=True,
                        output={"stdout": stdout, "stderr": stderr},
                        exit_code=exit_code,
                        stdout=stdout,
                        stderr=stderr,
                    )
                else:
                    return ExecutionResult(
                        success=False,
                        error=stderr or stdout or f"Command failed with exit code {exit_code}",
                        exit_code=exit_code,
                        stdout=stdout,
                        stderr=stderr,
                    )
            return self._parse_output(str(result), "", 0)
        except Exception:
            return None

    # ==================== Run / Exec Execution ====================

    def _build_run_command(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        input_data: Dict[str, Any],
    ) -> tuple:
        """Build command for skillbox run. Returns (cmd, stdin_data). Uses argv by default (L2 verified); stdin only when >100KB to avoid ARG_MAX."""
        abs_skill_dir = Path(skill_dir).resolve()
        input_str = json.dumps(input_data)
        # argv by default (L2 verified); stdin only for large input
        if len(input_str) > 100000:
            cmd = [self._binary_path, "run", str(abs_skill_dir), "-"]
            stdin_data = input_str.encode("utf-8")
        else:
            cmd = [self._binary_path, "run", str(abs_skill_dir), input_str]
            stdin_data = None
        
        cmd.extend(["--sandbox-level", context.sandbox_level])
        if context.allow_network:
            cmd.append("--allow-network")
        cmd.extend(["--timeout", str(context.timeout)])
        cmd.extend(["--max-memory", str(context.max_memory_mb)])
        
        return cmd, stdin_data
    
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

    def _get_ipc_client(self):
        """Get or create IPC client pool (lazy init)."""
        if self._ipc_client is None:
            from .skillbox.ipc_client import SkillboxIPCClientPool
            self._ipc_client = SkillboxIPCClientPool(
                binary_path=self._binary_path,
                cache_dir=None,
            )
        return self._ipc_client

    def _execute_via_ipc_run(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        input_data: Dict[str, Any],
    ) -> Optional[ExecutionResult]:
        """Execute run via IPC. Returns None on failure to fall back to subprocess."""
        try:
            client = self._get_ipc_client()
            result = client.run(
                skill_dir=str(Path(skill_dir).resolve()),
                input_json=json.dumps(input_data, ensure_ascii=False),
                allow_network=context.allow_network,
                cache_dir=None,
                timeout=context.timeout,
                max_memory=context.max_memory_mb,
                sandbox_level=context.sandbox_level,
            )
            output_str = result.get("output", "")
            exit_code = result.get("exit_code", 0)
            return self._parse_output(output_str, "", exit_code)
        except Exception:
            return None

    def _execute_via_ipc_exec(
        self,
        context: ExecutionContext,
        skill_dir: Path,
        script_path: str,
        input_data: Dict[str, Any],
        args: Optional[List[str]] = None,
    ) -> Optional[ExecutionResult]:
        """Execute exec via IPC. Returns None on failure to fall back to subprocess."""
        try:
            client = self._get_ipc_client()
            args_str = " ".join(args) if args else None
            result = client.exec_cmd(
                skill_dir=str(Path(skill_dir).resolve()),
                script_path=script_path,
                input_json=json.dumps(input_data, ensure_ascii=False),
                args=args_str,
                allow_network=context.allow_network,
                cache_dir=None,
                timeout=context.timeout,
                max_memory=context.max_memory_mb,
                sandbox_level=context.sandbox_level,
            )
            output_str = result.get("output", "")
            exit_code = result.get("exit_code", 0)
            return self._parse_output(output_str, "", exit_code)
        except Exception:
            return None

    def _run_subprocess(
        self,
        cmd: List[str],
        context: ExecutionContext,
        skill_dir: Path,
        stdin_data: Optional[bytes] = None,
    ) -> ExecutionResult:
        """Run subprocess with the given command."""
        env = self._build_env(context, skill_dir)

        use_text = stdin_data is None
        run_kw = dict(
            stdout=subprocess.PIPE,
            stderr=None,
            text=use_text,
            timeout=context.timeout,
            env=env,
        )
        if stdin_data is not None:
            run_kw["input"] = stdin_data
        if os.environ.get("SKILLBOX_DEBUG") == "1":
            import sys
            print(f"[skillbox] binary={self._binary_path}", file=sys.stderr, flush=True)
            print(f"[skillbox] spawning: {' '.join(cmd[:4])}...", file=sys.stderr, flush=True)
        try:
            result = subprocess.run(cmd, **run_kw)
            out = result.stdout if use_text else result.stdout.decode("utf-8", errors="replace")
            return self._parse_output(out, "", result.returncode)

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

        Delegates to shared env_utils.ensure_skill_python().
        """
        from ...env_utils import ensure_skill_python
        return ensure_skill_python(skill_dir)

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

        # Try to extract JSON from output using shared utility
        json_data = extract_json_from_output(combined, strategy="auto")
        if json_data is not None and isinstance(json_data, dict):
            return ExecutionResult(
                success=returncode == 0,
                output=json_data,
                exit_code=returncode,
                stdout=stdout,
                stderr=stderr,
            )

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
            # Format sandbox errors using shared utility
            formatted_error = format_sandbox_error(error_msg)
            return ExecutionResult(
                success=False,
                error=f"Skill execution failed with exit code {returncode}: {formatted_error}",
                exit_code=returncode,
                stdout=stdout,
                stderr=stderr,
            )


__all__ = ["UnifiedExecutor"]
