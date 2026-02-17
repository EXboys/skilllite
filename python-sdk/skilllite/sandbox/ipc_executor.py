"""
Thin IPC executor - direct ipc_client.run/exec for adapters (Phase 4.8 Strategy 7).

Replaces UnifiedExecutionService + UnifiedExecutor for LangChain/LlamaIndex adapters.
Security scan and confirmation are inlined; execution goes directly to ipc_client.
"""

import json
import os
from pathlib import Path
from typing import Any, Callable, Dict, Optional, TYPE_CHECKING

from .context import ExecutionResult
from .context import ExecutionContext

if TYPE_CHECKING:
    from ..core.skill_info import SkillInfo

ConfirmationCallback = Callable[[str, str], bool]


def execute_via_ipc(
    skill_info: "SkillInfo",
    input_data: Dict[str, Any],
    entry_point: Optional[str] = None,
    confirmation_callback: Optional[ConfirmationCallback] = None,
    allow_network: Optional[bool] = None,
    timeout: Optional[int] = None,
    sandbox_level: Optional[str] = None,
) -> ExecutionResult:
    """
    Execute skill via ipc_client directly (thin layer for adapters).

    Handles security scan + confirmation for L3, then calls ipc_client.run/exec.
    """
    context = ExecutionContext.from_current_env()
    if allow_network is not None or timeout is not None or sandbox_level is not None:
        context = context.with_override(
            allow_network=allow_network,
            timeout=timeout,
            sandbox_level=sandbox_level,
        )

    # Elevated permissions check
    if _requires_elevated(skill_info):
        context = context.with_elevated_permissions()

    # Security scan for L3
    if context.sandbox_level == "3":
        scan_result = _perform_scan(skill_info, input_data, entry_point)
        if scan_result and getattr(scan_result, "requires_confirmation", False):
            if confirmation_callback:
                report = getattr(scan_result, "format_report", lambda: str(scan_result))()
                scan_id = getattr(scan_result, "scan_id", "unknown")
                if not confirmation_callback(report, scan_id):
                    return ExecutionResult(
                        success=False,
                        error="Execution cancelled by user after security review",
                        exit_code=1,
                    )
            else:
                report = getattr(scan_result, "format_report", lambda: str(scan_result))()
                return ExecutionResult(
                    success=False,
                    error=f"Security confirmation required:\n{report}",
                    exit_code=2,
                )
        context = context.with_override(sandbox_level="2")

    # Direct ipc_client call
    return _execute(skill_info, input_data, entry_point, context)


def execute_bash_via_ipc(
    skill_info: "SkillInfo",
    command: str,
    timeout: Optional[int] = None,
) -> ExecutionResult:
    """Execute bash command via ipc_client directly."""
    context = ExecutionContext.from_current_env()
    if timeout is not None:
        context = context.with_override(timeout=timeout)
    return _execute_bash(skill_info, command, context)


def execute_with_context(
    context: ExecutionContext,
    skill_dir: Path,
    input_data: Dict[str, Any],
    entry_point: Optional[str] = None,
    args: Optional[list] = None,
) -> ExecutionResult:
    """
    Execute with explicit context (bypasses security scan).
    Used by SkillExecutor for programmatic execution.
    """
    return _execute_impl(
        skill_dir=skill_dir,
        input_data=input_data,
        entry_point=entry_point,
        args=args,
        context=context,
    )


def _execute(
    skill_info: "SkillInfo",
    input_data: Dict[str, Any],
    entry_point: Optional[str],
    context: ExecutionContext,
) -> ExecutionResult:
    """Call ipc_client.run or exec_cmd directly."""
    return _execute_impl(
        skill_dir=Path(skill_info.path),
        input_data=input_data,
        entry_point=entry_point,
        args=None,
        context=context,
    )


def _execute_impl(
    skill_dir: Path,
    input_data: Dict[str, Any],
    entry_point: Optional[str],
    args: Optional[list],
    context: ExecutionContext,
) -> ExecutionResult:
    """Internal: call ipc_client.run or exec_cmd."""
    try:
        pool = _get_ipc_pool()
        skill_dir_str = str(skill_dir.resolve())
        input_json = json.dumps(input_data, ensure_ascii=False)

        if entry_point:
            from .utils import convert_json_to_cli_args
            args_list = args if args is not None else convert_json_to_cli_args(input_data)
            args_str = " ".join(args_list) if args_list else None
            result = pool.exec_cmd(
                skill_dir=skill_dir_str,
                script_path=entry_point,
                input_json=input_json,
                args=args_str,
                allow_network=context.allow_network,
                timeout=context.timeout,
                max_memory=context.max_memory_mb,
                sandbox_level=context.sandbox_level,
            )
        else:
            result = pool.run(
                skill_dir=skill_dir_str,
                input_json=input_json,
                allow_network=context.allow_network,
                timeout=context.timeout,
                max_memory=context.max_memory_mb,
                sandbox_level=context.sandbox_level,
            )

        output_str = result.get("output", "")
        exit_code = result.get("exit_code", 0)
        return _parse_output(output_str, "", exit_code)
    except Exception as e:
        return ExecutionResult(success=False, error=str(e), exit_code=-1)


def _execute_bash(skill_info: "SkillInfo", command: str, context: ExecutionContext) -> ExecutionResult:
    """Execute bash via ipc_client."""
    try:
        pool = _get_ipc_pool()
        result = pool._get_client(timeout=context.timeout or 120)
        try:
            r = result._send_request(
                method="bash",
                params={
                    "skill_dir": str(Path(skill_info.path).resolve()),
                    "command": command,
                    "timeout": context.timeout or 120,
                    "cwd": os.getcwd(),
                },
            )
        finally:
            pool._return_client(result)

        if isinstance(r, str):
            try:
                r = json.loads(r)
            except (json.JSONDecodeError, TypeError):
                return ExecutionResult(success=False, error=r, exit_code=1)

        stdout = r.get("stdout", "")
        stderr = r.get("stderr", "")
        exit_code = r.get("exit_code", 0)
        if exit_code == 0:
            return ExecutionResult(
                success=True,
                output={"stdout": stdout, "stderr": stderr},
                exit_code=exit_code,
                stdout=stdout,
                stderr=stderr,
            )
        return ExecutionResult(
            success=False,
            error=stderr or stdout or f"Exit code {exit_code}",
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
        )
    except Exception as e:
        return ExecutionResult(success=False, error=str(e), exit_code=-1)


_ipc_pool = None


def _get_ipc_pool():
    """Get IPC client pool (lazy init, module singleton).
    Uses find_sandbox_binary() for run/exec/bash. Single-binary fallback: find_sandbox_binary() returns find_binary().
    Note: build_skills_context (agent) needs find_binary(); when using skilllite-sandbox, prompt_builder falls back to local.
    """
    global _ipc_pool
    if _ipc_pool is None:
        from .core import find_sandbox_binary
        from .core.ipc_client import SkillboxIPCClientPool
        binary_path = find_sandbox_binary()
        if not binary_path:
            raise RuntimeError("skilllite binary not found")
        _ipc_pool = SkillboxIPCClientPool(binary_path=binary_path, cache_dir=None)
    return _ipc_pool


def _parse_output(stdout: str, stderr: str, returncode: int) -> ExecutionResult:
    """Parse output into ExecutionResult."""
    from .utils import extract_json_from_output, format_sandbox_error
    combined = stdout + stderr
    json_data = extract_json_from_output(combined, strategy="auto")
    if json_data is not None and isinstance(json_data, dict):
        return ExecutionResult(
            success=returncode == 0,
            output=json_data,
            exit_code=returncode,
            stdout=stdout,
            stderr=stderr,
        )
    if returncode == 0:
        return ExecutionResult(
            success=True,
            output={"result": stdout.strip()} if stdout.strip() else None,
            exit_code=returncode,
            stdout=stdout,
            stderr=stderr,
        )
    formatted = format_sandbox_error(stderr.strip() or stdout.strip())
    return ExecutionResult(
        success=False,
        error=f"Skill execution failed with exit code {returncode}: {formatted}",
        exit_code=returncode,
        stdout=stdout,
        stderr=stderr,
    )


def _requires_elevated(skill_info: "SkillInfo") -> bool:
    if skill_info.metadata:
        return getattr(skill_info.metadata, "requires_elevated_permissions", False)
    return False


def _perform_scan(skill_info: "SkillInfo", input_data: Dict, entry_point: Optional[str]):
    """Run security scan. Returns SecurityScanResult."""
    try:
        from ..core.security import SecurityScanner
        scanner = SecurityScanner()
        return scanner.scan_skill(skill_info, input_data, entry_point=entry_point)
    except Exception:
        from ..core.security import SecurityScanResult
        return SecurityScanResult(
            is_safe=False,
            issues=[{
                "severity": "High",
                "issue_type": "Scan Error",
                "rule_id": "scan-exception",
                "line_number": 0,
                "description": "Security scan encountered an unexpected error.",
                "code_snippet": "",
            }],
            scan_id="error",
            code_hash="",
            high_severity_count=1,
        )
