"""
Skillbox executor - interfaces with the Rust skillbox binary.

This module provides the SkillboxExecutor class that implements the
SandboxExecutor interface using the Rust-based skillbox sandbox.

This is the canonical implementation of skill execution with sandbox support.
The core/executor.py module delegates to this class.
"""

import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

from ..base import SandboxExecutor, ExecutionResult
from ..config import (
    SandboxConfig,
    DEFAULT_EXECUTION_TIMEOUT,
    DEFAULT_MAX_MEMORY_MB,
    DEFAULT_SANDBOX_LEVEL,
    DEFAULT_ALLOW_NETWORK,
    DEFAULT_ENABLE_SANDBOX,
)
from ..utils import convert_json_to_cli_args, extract_json_from_output, format_sandbox_error
from .binary import find_binary, ensure_installed, _get_search_locations


class SkillboxExecutor(SandboxExecutor):
    """
    Executes skills using the skillbox binary.
    
    This class provides a Python interface to the Rust-based sandbox executor.
    Supports both traditional skill execution (via entry_point in SKILL.md) and
    direct script execution (via exec command).
    
    Features:
    - Sandbox security levels (1/2/3)
    - Configurable resource limits (memory, timeout)
    - Level 3 user interaction support for authorization prompts
    - Memory monitoring with psutil (optional)
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
        # Load configuration with environment variable fallbacks
        self._config = SandboxConfig(
            binary_path=binary_path,
            cache_dir=cache_dir,
            allow_network=allow_network if allow_network else DEFAULT_ALLOW_NETWORK,
            enable_sandbox=enable_sandbox if enable_sandbox else DEFAULT_ENABLE_SANDBOX,
            execution_timeout=execution_timeout if execution_timeout is not None else DEFAULT_EXECUTION_TIMEOUT,
            max_memory_mb=max_memory_mb if max_memory_mb is not None else DEFAULT_MAX_MEMORY_MB,
            sandbox_level=sandbox_level if sandbox_level is not None else DEFAULT_SANDBOX_LEVEL,
            auto_install=auto_install,
        )
        
        # Set instance attributes from config
        self.binary_path = self._config.binary_path or self._find_binary(auto_install)
        self.cache_dir = self._config.cache_dir
        self.allow_network = self._config.allow_network
        self.enable_sandbox = self._config.enable_sandbox
        self.execution_timeout = self._config.execution_timeout
        self.max_memory_mb = self._config.max_memory_mb
        self.sandbox_level = self._config.sandbox_level
    
    def _find_binary(self, auto_install: bool = False) -> str:
        """
        Find the skillbox binary.
        
        Uses the binary module to search for the binary in standard locations.
        If auto_install is True, will download and install if not found.
        
        Args:
            auto_install: Automatically install if not found.
            
        Returns:
            Path to the binary.
            
        Raises:
            FileNotFoundError: If binary not found and auto_install is False.
            PermissionError: If permission denied when accessing binary.
            RuntimeError: If failed to find or install binary.
        """
        try:
            return ensure_installed(auto_install=auto_install, show_progress=True)
        except FileNotFoundError as e:
            searched = _get_search_locations()
            paths_info = "\n".join(f"    - [{label}] {path}" for label, path in searched)
            raise FileNotFoundError(
                f"Skillbox binary not found. Searched locations:\n"
                f"{paths_info}\n\n"
                f"Please run 'cargo build --release' in skillbox/ directory, "
                f"or set SKILLBOX_BINARY_PATH environment variable to the binary path."
            ) from e
        except PermissionError as e:
            raise PermissionError(
                f"Permission denied when accessing skillbox binary. "
                f"Please check file permissions. Original error: {e}"
            ) from e
        except Exception as e:
            raise RuntimeError(
                f"Failed to find or install skillbox binary: {e}. "
                f"Please ensure Rust is installed and run 'cargo build --release' in skillbox/ directory."
            ) from e
    
    @property
    def is_available(self) -> bool:
        """Check if skillbox is available and ready to use."""
        if not self.binary_path:
            return False
        return os.path.exists(self.binary_path) and os.access(self.binary_path, os.X_OK)
    
    @property
    def name(self) -> str:
        """Return the name of this sandbox implementation."""
        return "skillbox"
    
    def _convert_json_to_cli_args(self, input_data: Dict[str, Any]) -> List[str]:
        """
        Convert JSON input data to command line arguments list.
        
        Delegates to the shared utility function in sandbox.utils.
        
        Args:
            input_data: JSON input data from LLM
            
        Returns:
            List of command line arguments
        """
        return convert_json_to_cli_args(input_data)
    
    def _build_skill_env(self, skill_dir: Path, timeout: Optional[int] = None) -> Dict[str, str]:
        """
        Build environment variables for skill execution.

        Args:
            skill_dir: Path to the skill directory
            timeout: Optional timeout override

        Returns:
            Environment dictionary
        """
        # Allow runtime override of sandbox level via environment variable
        # This enables Python-layer security confirmation to skip skillbox confirmation
        effective_sandbox_level = os.environ.get("SKILLBOX_SANDBOX_LEVEL", self.sandbox_level)

        return {
            **os.environ,
            "PYTHONUNBUFFERED": "1",
            "SKILL_DIR": str(skill_dir),
            "SKILL_ASSETS_DIR": str(skill_dir / "assets"),
            "SKILL_REFERENCES_DIR": str(skill_dir / "references"),
            "SKILL_SCRIPTS_DIR": str(skill_dir / "scripts"),
            "SKILLBOX_SANDBOX_LEVEL": effective_sandbox_level,
            "SKILLBOX_MAX_MEMORY_MB": str(self.max_memory_mb),
            "SKILLBOX_TIMEOUT_SECS": str(timeout if timeout is not None else self.execution_timeout),
            "SKILLBOX_AUTO_APPROVE": os.environ.get("SKILLBOX_AUTO_APPROVE", ""),
        }

    def _format_sandbox_error(self, error_msg: str) -> str:
        """
        Format sandbox restriction errors into user-friendly messages.

        Delegates to shared utility function in sandbox.utils.

        Args:
            error_msg: Raw error message from subprocess

        Returns:
            Formatted error message
        """
        return format_sandbox_error(error_msg)

    def _extract_json_from_output(self, output: str) -> Optional[Any]:
        """
        Try to extract JSON from skillbox output that may contain log lines.

        Delegates to shared utility function in sandbox.utils.

        Skillbox output format may include:
        - [INFO] ... log lines
        - [WARN] ... log lines
        - Error: ... messages
        - Then the actual JSON object

        Args:
            output: Raw output from skillbox

        Returns:
            Parsed JSON object if found, None otherwise
        """
        return extract_json_from_output(output, strategy="auto")

    def _parse_output(self, stdout: str, stderr: str, returncode: int) -> ExecutionResult:
        """
        Parse subprocess output into ExecutionResult.
        
        Uses shared utility functions for JSON extraction and error formatting.
        
        Args:
            stdout: Standard output
            stderr: Standard error
            returncode: Process return code
            
        Returns:
            ExecutionResult
        """
        if returncode == 0:
            # Try to extract JSON from output using shared utility
            json_data = extract_json_from_output(stdout, strategy="auto")
            if json_data is not None:
                return ExecutionResult(
                    success=True,
                    output=json_data,
                    exit_code=returncode,
                    stdout=stdout,
                    stderr=stderr
                )
            # Script output is not JSON, return as raw text
            return ExecutionResult(
                success=True,
                output={"raw_output": stdout.strip()} if stdout.strip() else None,
                exit_code=returncode,
                stdout=stdout,
                stderr=stderr
            )
        else:
            # Check for sandbox restriction errors and provide friendly messages
            error_msg = stderr or stdout or f"Exit code: {returncode}"
            error_msg = self._format_sandbox_error(error_msg)

            return ExecutionResult(
                success=False,
                error=error_msg,
                exit_code=returncode,
                stdout=stdout,
                stderr=stderr
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
        
        Note: All execution goes through skillbox for consistent dependency resolution
        and sandbox behavior across all security levels.
        
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
        if allow_network is None:
            allow_network = self.allow_network
        
        if enable_sandbox is None:
            enable_sandbox = self.enable_sandbox
        
        # Convert JSON input to CLI args if no explicit args provided
        if args is None and input_data:
            args = self._convert_json_to_cli_args(input_data)
        
        full_script_path = skill_dir / script_path
        
        # All levels (1, 2, 3) go through skillbox for:
        # - Consistent dependency resolution (ensure_environment reads compatibility/lock)
        # - Level 2: proper sandbox isolation (macOS Seatbelt / Linux namespace)
        # - Level 1: resource limits and env_path with deps (no isolation)
        cmd = [
            self.binary_path,
            "exec",
            str(skill_dir),
            script_path,
            json.dumps(input_data),
        ]
        
        if args:
            args_str = " ".join(args) if isinstance(args, list) else args
            cmd.extend(["--args", args_str])
        
        if allow_network:
            cmd.append("--allow-network")

        # Use --sandbox-level instead of --enable-sandbox
        # sandbox_level: 1=no sandbox, 2=sandbox only, 3=sandbox+scan
        if self.sandbox_level:
            cmd.extend(["--sandbox-level", str(self.sandbox_level)])

        if self.cache_dir:
            cmd.extend(["--cache-dir", self.cache_dir])
        
        # Add resource limits
        effective_timeout = timeout if timeout is not None else self.execution_timeout
        cmd.extend([
            "--timeout", str(effective_timeout),
            "--max-memory", str(self.max_memory_mb)
        ])
        
        skill_env = self._build_skill_env(skill_dir, timeout)
        
        # Execute with Level 3 user interaction support
        try:
            if self.sandbox_level == "3":
                # Level 3: Allow user interaction for authorization prompts
                result = subprocess.run(
                    cmd,
                    stdin=None,
                    stdout=subprocess.PIPE,
                    stderr=None,  # Let stderr flow to terminal for authorization prompts
                    text=True,
                    timeout=effective_timeout,
                    env=skill_env
                )
                return self._parse_output(result.stdout, "", result.returncode)
            else:
                # 捕获 stdout 解析 JSON，stderr 实时输出到终端便于看到 playwright 等进度
                result = subprocess.run(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=None,
                    text=True,
                    timeout=effective_timeout,
                    env=skill_env
                )
                return self._parse_output(result.stdout, "", result.returncode)
                
        except subprocess.TimeoutExpired:
            return ExecutionResult(
                success=False,
                error=f"Execution timed out after {effective_timeout} seconds",
                exit_code=-1
            )
        except FileNotFoundError:
            return ExecutionResult(
                success=False,
                error=f"skillbox binary not found at: {self.binary_path}",
                exit_code=-1
            )
        except Exception as e:
            return ExecutionResult(
                success=False,
                error=f"Execution failed: {str(e)}",
                exit_code=-1
            )
    
    def _ensure_skill_python(self, skill_dir: Path) -> str:
        """Get Python executable with dependencies installed if needed.

        Mirrors the logic in UnifiedExecutor._ensure_skill_python.
        """
        try:
            from ...core.metadata import parse_skill_metadata
            from ...cli.init import (
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

        packages = metadata.resolved_packages
        if packages is None:
            packages = parse_compatibility_for_packages(metadata.compatibility)

        if not packages:
            return sys.executable

        language = metadata.language or "python"
        content_hash = _compute_packages_hash(packages)
        cache_key = _get_cache_key(language, content_hash)
        cache_dir = _get_cache_dir()
        cache_dir.mkdir(parents=True, exist_ok=True)
        env_path = cache_dir / cache_key

        _ensure_python_env(env_path, packages)

        python = (
            env_path / "Scripts" / "python"
            if os.name == "nt"
            else env_path / "bin" / "python"
        )
        return str(python) if python.exists() else sys.executable

    def _exec_python_script_direct(
        self,
        skill_dir: Path,
        script_path: Path,
        input_data: Dict[str, Any],
        args: Optional[list],
        timeout: Optional[int]
    ) -> ExecutionResult:
        """
        Execute Python script directly using subprocess with memory monitoring.
        
        This provides better argument handling for CLI-style Python scripts
        that use argparse or sys.argv, and adds memory limit protection.
        
        Args:
            skill_dir: Path to the skill directory
            script_path: Full path to the script
            input_data: Input data (for reference)
            args: Command line arguments
            timeout: Execution timeout in seconds
            
        Returns:
            ExecutionResult with the output or error
        """
        effective_timeout = timeout if timeout is not None else self.execution_timeout
        
        # Try to import psutil for memory monitoring
        try:
            import psutil
            has_psutil = True
        except ImportError:
            has_psutil = False
        
        # Ensure dependencies are installed and get the correct python executable
        python_executable = self._ensure_skill_python(skill_dir)
        cmd = [python_executable, str(script_path)]
        
        if args:
            cmd.extend(args)
        
        skill_env = self._build_skill_env(skill_dir, timeout)
        
        # Memory monitoring variables
        memory_limit_bytes = self.max_memory_mb * 1024 * 1024
        memory_exceeded = threading.Event()
        memory_monitor_error: List[str] = []
        
        def monitor_memory(proc: subprocess.Popen) -> None:
            """Monitor process memory usage in a separate thread."""
            if not has_psutil:
                return
            try:
                import psutil as ps
                ps_process = ps.Process(proc.pid)
                while not memory_exceeded.is_set() and proc.poll() is None:
                    try:
                        mem_info = ps_process.memory_info()
                        if mem_info.rss > memory_limit_bytes:
                            memory_monitor_error.append(
                                f"Process killed: memory usage ({mem_info.rss / (1024*1024):.2f} MB) "
                                f"exceeded limit ({self.max_memory_mb} MB)"
                            )
                            memory_exceeded.set()
                            proc.terminate()
                            break
                    except (ps.NoSuchProcess, ps.AccessDenied):
                        break
                    time.sleep(0.1)
            except Exception:
                pass
        
        try:
            # Pass input data via stdin for scripts that read from stdin
            input_json = json.dumps(input_data, ensure_ascii=False)
            
            proc = subprocess.Popen(
                cmd,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=skill_env
            )
            
            # Start memory monitoring thread
            if has_psutil:
                monitor_thread = threading.Thread(target=monitor_memory, args=(proc,), daemon=True)
                monitor_thread.start()
            
            try:
                stdout, stderr = proc.communicate(input=input_json, timeout=effective_timeout)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.communicate()
                memory_exceeded.set()
                return ExecutionResult(
                    success=False,
                    error=f"Execution timed out after {effective_timeout} seconds",
                    exit_code=-1
                )
            
            # Stop memory monitoring
            memory_exceeded.set()
            
            # Check if process was killed due to memory limit
            if memory_monitor_error:
                return ExecutionResult(
                    success=False,
                    error=memory_monitor_error[0],
                    exit_code=-1
                )
            
            return self._parse_output(stdout, stderr, proc.returncode)
                
        except Exception as e:
            memory_exceeded.set()
            return ExecutionResult(
                success=False,
                error=f"Execution failed: {str(e)}",
                exit_code=-1
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
        # If a specific entry_point is provided, use exec_script
        if entry_point:
            return self.exec_script(
                skill_dir=skill_dir,
                script_path=entry_point,
                input_data=input_data,
                allow_network=allow_network,
                timeout=timeout,
                enable_sandbox=enable_sandbox
            )
        
        if allow_network is None:
            allow_network = self.allow_network
        
        if enable_sandbox is None:
            enable_sandbox = self.enable_sandbox
        
        effective_timeout = timeout if timeout is not None else self.execution_timeout
        
        # All levels (1, 2, 3) go through skillbox for consistent dependency resolution
        # and sandbox behavior (Level 2 = isolation, Level 1 = env_path with deps only)
        
        # Build command for skillbox run
        cmd = [
            self.binary_path,
            "run",
            str(skill_dir),
            json.dumps(input_data)
        ]
        
        if allow_network:
            cmd.append("--allow-network")
        
        if self.cache_dir:
            cmd.extend(["--cache-dir", self.cache_dir])
        
        skill_env = self._build_skill_env(skill_dir, timeout)

        # Get effective sandbox level (may be overridden by environment variable)
        effective_sandbox_level = skill_env.get("SKILLBOX_SANDBOX_LEVEL", self.sandbox_level)

        # Execute with Level 3 user interaction support
        try:
            if effective_sandbox_level == "3":
                # Level 3: Allow user interaction for authorization prompts
                result = subprocess.run(
                    cmd,
                    stdin=None,
                    stdout=subprocess.PIPE,
                    stderr=None,  # Let stderr flow to terminal for authorization prompts
                    text=True,
                    timeout=effective_timeout,
                    env=skill_env
                )
            else:
                # Level 1/2: Capture all output
                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=effective_timeout,
                    env=skill_env
                )
            
            stderr = result.stderr if hasattr(result, 'stderr') and result.stderr else ""

            # Combine stdout and stderr for JSON extraction
            # skillbox may output JSON to either stream
            combined_output = result.stdout + stderr

            if result.returncode == 0:
                output = self._extract_json_from_output(combined_output)
                if output is not None:
                    return ExecutionResult(
                        success=True,
                        output=output,
                        exit_code=result.returncode,
                        stdout=result.stdout,
                        stderr=stderr
                    )
                else:
                    # No JSON found, return raw output as success
                    return ExecutionResult(
                        success=True,
                        output={"raw_output": result.stdout.strip()},
                        exit_code=result.returncode,
                        stdout=result.stdout,
                        stderr=stderr
                    )
            else:
                # Check if there's valid JSON in combined output (skillbox may still output results)
                output = self._extract_json_from_output(combined_output)
                if output is not None:
                    # If we found valid JSON with exit_code=0, treat as success
                    if isinstance(output, dict) and output.get("exit_code") == 0:
                        return ExecutionResult(
                            success=True,
                            output=output,
                            exit_code=0,
                            stdout=result.stdout,
                            stderr=stderr
                        )

                error_msg = stderr or result.stdout or f"Exit code: {result.returncode}"
                error_msg = self._format_sandbox_error(error_msg)

                return ExecutionResult(
                    success=False,
                    error=error_msg,
                    exit_code=result.returncode,
                    stdout=result.stdout,
                    stderr=stderr
                )
                
        except subprocess.TimeoutExpired:
            return ExecutionResult(
                success=False,
                error=f"Execution timed out after {effective_timeout} seconds",
                exit_code=-1
            )
        except FileNotFoundError:
            return ExecutionResult(
                success=False,
                error=f"skillbox binary not found at: {self.binary_path}",
                exit_code=-1
            )
        except Exception as e:
            return ExecutionResult(
                success=False,
                error=str(e),
                exit_code=-1
            )
