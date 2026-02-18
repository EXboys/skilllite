"""
API: scan_code, execute_code, chat — Python ↔ binary bridge.

- scan_code / execute_code: IDE/MCP integration (sandbox focus)
- chat: Agent chat (single-shot or interactive) — hides binary CLI details
"""

import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Dict, Optional

_EXT = {"python": ".py", "javascript": ".js", "bash": ".sh"}


def scan_code(language: str, code: str) -> Dict[str, Any]:
    """
    Scan code for security issues.

    Args:
        language: python, javascript, bash
        code: Code to scan

    Returns:
        Dict with is_safe, issues, requires_confirmation, scan_id, etc.
    """
    from .binary import get_binary

    binary = get_binary()
    if not binary:
        return {
            "is_safe": False,
            "issues": [{"message": "skilllite binary not found"}],
            "requires_confirmation": False,
            "error": "Binary not found. Run: pip install skilllite",
        }

    ext = _EXT.get(language, ".py")
    tmpdir = Path(tempfile.mkdtemp(prefix="skilllite_scan_", dir=os.getcwd()))
    tmp_path = tmpdir / f"script{ext}"
    tmp_path.write_text(code, encoding="utf-8")
    try:
        result = subprocess.run(
            [binary, "security-scan", str(tmp_path), "--json"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        try:
            data = json.loads(result.stdout) if result.stdout else {}
            return {
                "is_safe": data.get("is_safe", False),
                "issues": data.get("issues", []),
                "requires_confirmation": data.get("high_severity_count", 0) > 0,
                "scan_id": data.get("scan_id", ""),
            }
        except json.JSONDecodeError:
            return {
                "is_safe": False,
                "issues": [{"message": result.stderr or result.stdout or "Scan failed"}],
                "requires_confirmation": False,
            }
    finally:
        import shutil
        shutil.rmtree(tmpdir, ignore_errors=True)


def execute_code(
    language: str,
    code: str,
    *,
    confirmed: bool = False,
    scan_id: Optional[str] = None,
    sandbox_level: int = 3,
) -> Dict[str, Any]:
    """
    Execute code in sandbox.

    Args:
        language: python, javascript, bash
        code: Code to execute
        confirmed: True if user confirmed after security scan (ignored for L1/L2)
        scan_id: Scan ID from scan_code (required when confirmed=True)
        sandbox_level: 1=no sandbox, 2=sandbox only, 3=sandbox+scan

    Returns:
        Dict with success, stdout, stderr, exit_code, text
    """
    ext = _EXT.get(language, ".py")
    script_name = "main" + ext
    with tempfile.TemporaryDirectory(prefix="skilllite_exec_", dir=os.getcwd()) as tmpdir:
        script_path = Path(tmpdir) / script_name
        script_path.write_text(code, encoding="utf-8")

        # Use IPC when SKILLBOX_USE_IPC=1 (avoids process startup per call)
        from .ipc import _get_client

        client = _get_client()
        if client:
            try:
                res = client.exec(
                    str(tmpdir),
                    script_name,
                    "{}",
                    sandbox_level=sandbox_level,
                )
                output = res.get("output", "")
                exit_code = res.get("exit_code", 0)
                return {
                    "success": exit_code == 0,
                    "stdout": output,
                    "stderr": "",
                    "exit_code": exit_code,
                    "text": output,
                }
            except Exception as e:
                return {
                    "success": False,
                    "stdout": "",
                    "stderr": str(e),
                    "exit_code": 1,
                    "text": str(e),
                }

        # Fallback: subprocess
        from .binary import get_binary

        binary = get_binary()
        if not binary:
            return {
                "success": False,
                "text": "skilllite binary not found. Run: pip install skilllite",
                "exit_code": 1,
            }
        result = subprocess.run(
            [
                binary,
                "exec",
                tmpdir,
                script_name,
                "{}",
                "--sandbox-level",
                str(sandbox_level),
            ],
            capture_output=True,
            text=True,
            timeout=60,
            cwd=tmpdir,
        )
    return {
        "success": result.returncode == 0,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.returncode,
        "text": result.stdout + (result.stderr or ""),
    }


def run_skill(
    skill_dir: str,
    input_json: str,
    *,
    sandbox_level: int = 3,
    allow_network: bool = False,
    auto_approve: bool = False,
) -> Dict[str, Any]:
    """
    Run a skill with the given input JSON.

    Args:
        skill_dir: Path to the skill directory (must contain SKILL.md with entry_point)
        input_json: JSON string for skill input (e.g. '{"name": "Alice"}')
        sandbox_level: 1=no sandbox, 2=sandbox only, 3=sandbox+scan
        allow_network: Whether to allow network access
        auto_approve: If True, set SKILLBOX_AUTO_APPROVE=1 (skip security confirmation prompt)

    Returns:
        Dict with success, stdout, stderr, exit_code, text
    """
    from .binary import get_binary

    binary = get_binary()
    if not binary:
        return {
            "success": False,
            "stdout": "",
            "stderr": "skilllite binary not found. Run: pip install skilllite",
            "exit_code": 1,
            "text": "skilllite binary not found. Run: pip install skilllite",
        }

    cmd = [
        binary,
        "run",
        skill_dir,
        input_json,
        "--sandbox-level",
        str(sandbox_level),
    ]
    if allow_network:
        cmd.append("--allow-network")

    env = dict(os.environ)
    if auto_approve:
        env["SKILLBOX_AUTO_APPROVE"] = "1"

    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=60,
        cwd=os.getcwd(),
        env=env,
    )

    text = (result.stdout or "") + (result.stderr or "")
    return {
        "success": result.returncode == 0,
        "stdout": result.stdout or "",
        "stderr": result.stderr or "",
        "exit_code": result.returncode,
        "text": text.strip(),
    }


def chat(
    message: str,
    *,
    skills_dir: str = ".skills",
    workspace: Optional[str] = None,
    max_iterations: int = 50,
    verbose: bool = True,
    session: str = "default",
    model: Optional[str] = None,
    cwd: Optional[str] = None,
    env: Optional[Dict[str, str]] = None,
    stream: bool = True,
) -> Dict[str, Any]:
    """
    Run agent chat (single-shot mode). Bridges Python → binary without exposing CLI.

    Args:
        message: User message to send
        skills_dir: Skills directory (default: .skills)
        workspace: Workspace directory (default: cwd)
        max_iterations: Max agent loop iterations (default: 50)
        verbose: Verbose output (default: True)
        session: Session key for persistent conversation (default: default)
        model: Model name override (uses env if not set)
        cwd: Working directory for subprocess (default: current dir)
        env: Environment overrides (merged with os.environ)
        stream: If True, output goes to terminal; if False, captured in return dict

    Returns:
        Dict with success, exit_code; stdout/stderr only when stream=False
    """
    from .binary import get_binary

    binary = get_binary()
    if not binary:
        return {
            "success": False,
            "exit_code": 1,
            "stdout": "",
            "stderr": "skilllite binary not found. Run: pip install skilllite",
        }

    cmd = [
        binary,
        "chat",
        "--message",
        message,
        "-s",
        skills_dir,
        "--max-iterations",
        str(max_iterations),
        "--session",
        session,
    ]
    if verbose:
        cmd.append("--verbose")
    if workspace:
        cmd.extend(["--workspace", workspace])
    if model:
        cmd.extend(["--model", model])

    run_env = dict(os.environ)
    if env:
        run_env.update(env)

    result = subprocess.run(
        cmd,
        capture_output=not stream,
        text=True,
        timeout=300,
        cwd=cwd or os.getcwd(),
        env=run_env,
    )

    out = {
        "success": result.returncode == 0,
        "exit_code": result.returncode,
    }
    if not stream:
        out["stdout"] = result.stdout or ""
        out["stderr"] = result.stderr or ""
    return out
