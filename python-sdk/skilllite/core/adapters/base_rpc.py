"""
RPC-based Adapter Base - Uses list_tools_with_meta + run/exec/bash RPC.

No SkillManager dependency. For LangChain/LlamaIndex adapters that want
to avoid ToolBuilder, PromptBuilder, ToolCallHandler.
"""

import json
import os
import subprocess
import time
import uuid
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, TYPE_CHECKING

from ..security import (
    SecurityScanResult,
    ConfirmationCallback,
    AsyncConfirmationCallback,
    parse_scan_json_output,
)

if TYPE_CHECKING:
    from ...sandbox.context import ExecutionResult


class RpcAdapter(ABC):
    """
    Base adapter that uses list_tools_with_meta + run/exec/bash RPC.
    No SkillManager, ToolBuilder, PromptBuilder, ToolCallHandler.
    """

    _scan_cache: Dict[str, SecurityScanResult] = {}
    _confirmed_skills: Dict[str, float] = {}
    _SCAN_CACHE_TTL: int = 300
    _CONFIRMATION_TTL: int = 3600

    def __init__(
        self,
        skills_dir: str,
        sandbox_level: int = 3,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        confirmation_callback: Optional[ConfirmationCallback] = None,
        async_confirmation_callback: Optional[AsyncConfirmationCallback] = None,
        skill_names: Optional[List[str]] = None,
    ):
        self.skills_dir = str(Path(skills_dir).resolve())
        self.sandbox_level = sandbox_level
        self.allow_network = allow_network
        self.timeout = timeout
        self.confirmation_callback = confirmation_callback
        self.async_confirmation_callback = async_confirmation_callback
        self.skill_names = skill_names
        self._tools: List[Dict[str, Any]] = []
        self._tool_meta: Dict[str, Dict[str, Any]] = {}
        self._load_tools()

    def _load_tools(self) -> None:
        """Load tools and tool_meta via list_tools_with_meta RPC."""
        from ...sandbox.core import find_binary, find_sandbox_binary
        from ...sandbox.core.ipc_client import SkillboxIPCClientPool

        binary = find_binary()
        if not binary:
            raise RuntimeError("skilllite binary not found")
        pool = SkillboxIPCClientPool(binary_path=binary)
        try:
            data = pool.list_tools_with_meta(
                skills_dir=self.skills_dir,
                skills=self.skill_names,
                format="openai",
            )
            self._tools = data.get("tools", [])
            self._tool_meta = data.get("tool_meta", {})
        finally:
            pool.close()

    def execute_tool(
        self,
        tool_name: str,
        input_data: Dict[str, Any],
    ) -> "ExecutionResult":
        """Execute tool via run/exec/bash RPC using tool_meta."""
        from ...sandbox.context import ExecutionResult
        from ...sandbox.core import find_sandbox_binary
        from ...sandbox.core.ipc_client import SkillboxIPCClientPool
        from ...sandbox.utils import extract_json_from_output, format_sandbox_error

        meta = self._tool_meta.get(tool_name)
        if not meta:
            return ExecutionResult(
                success=False,
                error=f"Tool '{tool_name}' not found in tool_meta",
                exit_code=1,
            )

        skill_dir = meta.get("skill_dir", "")
        script_path = meta.get("script_path")
        entry_point = meta.get("entry_point")
        is_bash = meta.get("is_bash", False)

        # Security scan for L3
        if self.sandbox_level == 3:
            scan_result = self._perform_scan(tool_name, meta, input_data)
            if scan_result and getattr(scan_result, "requires_confirmation", False):
                if self.confirmation_callback:
                    report = getattr(scan_result, "format_report", lambda: str(scan_result))()
                    scan_id = getattr(scan_result, "scan_id", "unknown")
                    if not self.confirmation_callback(report, scan_id):
                        return ExecutionResult(
                            success=False,
                            error="Execution cancelled by user",
                            exit_code=1,
                        )
                else:
                    report = getattr(scan_result, "format_report", lambda: str(scan_result))()
                    return ExecutionResult(
                        success=False,
                        error=f"Security confirmation required:\n{report}",
                        exit_code=2,
                    )

        binary = find_sandbox_binary()
        if not binary:
            return ExecutionResult(success=False, error="skilllite binary not found", exit_code=1)

        pool = SkillboxIPCClientPool(binary_path=binary)
        try:
            if is_bash:
                cmd = input_data.get("command", "")
                if not cmd:
                    return ExecutionResult(
                        success=False,
                        error="Bash tool requires 'command' parameter",
                        exit_code=1,
                    )
                r = pool.bash(
                    skill_dir=skill_dir,
                    command=cmd,
                    timeout=self.timeout or 120,
                    cwd=os.getcwd(),
                )
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

            input_json = json.dumps(input_data, ensure_ascii=False)
            if script_path:
                result = pool.exec_cmd(
                    skill_dir=skill_dir,
                    script_path=script_path,
                    input_json=input_json,
                    allow_network=self.allow_network,
                    timeout=self.timeout,
                    sandbox_level=str(self.sandbox_level) if self.sandbox_level < 3 else "2",
                )
            else:
                result = pool.run(
                    skill_dir=skill_dir,
                    input_json=input_json,
                    allow_network=self.allow_network,
                    timeout=self.timeout,
                    sandbox_level=str(self.sandbox_level) if self.sandbox_level < 3 else "2",
                )

            output_str = result.get("output", "")
            exit_code = result.get("exit_code", 0)
            json_data = extract_json_from_output(str(output_str), strategy="auto")
            if json_data is not None and isinstance(json_data, dict):
                return ExecutionResult(
                    success=exit_code == 0,
                    output=json_data,
                    exit_code=exit_code,
                )
            if exit_code == 0:
                return ExecutionResult(
                    success=True,
                    output={"result": output_str.strip()} if output_str.strip() else None,
                    exit_code=exit_code,
                )
            return ExecutionResult(
                success=False,
                error=format_sandbox_error(output_str.strip()),
                exit_code=exit_code,
            )
        finally:
            pool.close()

    def _perform_scan(
        self,
        tool_name: str,
        meta: Dict[str, Any],
        input_data: Dict[str, Any],
    ) -> Optional[SecurityScanResult]:
        """Run security-scan on script. Returns SecurityScanResult."""
        skill_dir = meta.get("skill_dir", "")
        script_path = meta.get("script_path")
        entry_point = meta.get("entry_point")

        script_to_scan = None
        if script_path:
            script_to_scan = Path(skill_dir) / script_path
        elif entry_point:
            script_to_scan = Path(skill_dir) / entry_point

        if not script_to_scan or not script_to_scan.exists():
            return SecurityScanResult.safe(str(uuid.uuid4()), "no-script")

        try:
            from ...sandbox.core import find_sandbox_binary
            binary = find_sandbox_binary()
            if not binary:
                return None
            result = subprocess.run(
                [binary, "security-scan", "--json", str(script_to_scan)],
                capture_output=True,
                text=True,
                timeout=30,
            )
            data = parse_scan_json_output(result.stdout)
            return SecurityScanResult(
                is_safe=data["is_safe"],
                issues=data["issues"],
                scan_id=str(uuid.uuid4()),
                code_hash="",
                high_severity_count=data["high_severity_count"],
                medium_severity_count=data["medium_severity_count"],
                low_severity_count=data["low_severity_count"],
            )
        except Exception:
            return None

    @abstractmethod
    def to_tools(self) -> List[Any]:
        """Convert to framework-specific tools."""
        pass


__all__ = ["RpcAdapter"]
