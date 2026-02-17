"""
Thin IPC client for skillbox agent-rpc (JSON-Lines event stream over stdio).

Replaces Python AgenticLoop/ChatSession/TaskPlanner/BuiltinTools with a thin
wrapper that delegates to the Rust agent engine.

Protocol:
  Request (stdin):  {"method": "agent_chat", "params": {"message": "...", ...}}
  Response (stdout): {"event": "text", "data": {...}}  (streaming events)
"""

import json
import os
import subprocess
import threading
from pathlib import Path
from typing import Any, Callable, Dict, Generator, List, Optional

from .binary import find_binary, ensure_installed


def _get_binary() -> str:
    """Get skillbox binary path (ensure installed for agent-rpc)."""
    return ensure_installed(auto_install=True)


def _prepare_agent_env(
    env: Dict[str, str],
    api_key: Optional[str],
    api_base: Optional[str],
    model: Optional[str],
    confirmation_callback: Optional[Callable[[str, str], bool]],
) -> None:
    """Prepare env for agent-rpc subprocess."""
    if api_key:
        env["OPENAI_API_KEY"] = api_key
    if api_base:
        env["OPENAI_API_BASE"] = api_base
    if model:
        env["SKILLLITE_MODEL"] = model
    # When confirmation_callback is provided, Python handles confirmation via events.
    # Set SKILLBOX_AUTO_APPROVE so the executor (second confirmation layer) does not
    # block on stdin.read_line() - that would deadlock since stdin is a pipe.
    if confirmation_callback:
        env["SKILLBOX_AUTO_APPROVE"] = "1"
    # Allow Playwright skills (e.g. xiaohongshu-writer) - sandbox blocks process-exec.
    # In interactive mode, default allow; set SKILLBOX_ALLOW_PLAYWRIGHT=0 to disable.
    if confirmation_callback and os.environ.get("SKILLBOX_ALLOW_PLAYWRIGHT", "1").strip().lower() not in ("0", "false", "no"):
        env["SKILLBOX_ALLOW_PLAYWRIGHT"] = "1"


def agent_chat(
    message: str,
    *,
    session_key: str = "default",
    skill_dirs: Optional[List[str]] = None,
    workspace: Optional[str] = None,
    model: Optional[str] = None,
    api_base: Optional[str] = None,
    api_key: Optional[str] = None,
    max_iterations: Optional[int] = None,
    enable_task_planning: Optional[bool] = None,
    stream_callback: Optional[Callable[[str], None]] = None,
    confirmation_callback: Optional[Callable[[str, str], bool]] = None,
) -> str:
    """
    Run agent chat via skillbox agent-rpc. Returns final response text.

    Args:
        message: User input message.
        session_key: Session key for persistent conversation.
        skill_dirs: Skill directories to load (default: auto-discover from workspace).
        workspace: Workspace path (default: current dir).
        model: Model override (default: from env).
        api_base: API base URL override.
        api_key: API key override.
        max_iterations: Max agent loop iterations.
        enable_task_planning: Enable task planning.
        stream_callback: Callback for streaming text chunks.
        confirmation_callback: Callback for security confirmation (scan_id, report) -> bool.

    Returns:
        Final assistant response text.
    """
    binary = _get_binary()
    env = os.environ.copy()
    _prepare_agent_env(env, api_key, api_base, model, confirmation_callback)

    params: Dict[str, Any] = {
        "message": message,
        "session_key": session_key,
    }
    if skill_dirs:
        params["skill_dirs"] = skill_dirs
    if workspace:
        params["config"] = params.get("config", {})
        params["config"]["workspace"] = workspace
    if model:
        params.setdefault("config", {})["model"] = model
    if api_base:
        params.setdefault("config", {})["api_base"] = api_base
    if api_key:
        params.setdefault("config", {})["api_key"] = api_key
    if max_iterations is not None:
        params.setdefault("config", {})["max_iterations"] = max_iterations
    if enable_task_planning is not None:
        params.setdefault("config", {})["enable_task_planning"] = enable_task_planning

    request = {"method": "agent_chat", "params": params}
    request_line = json.dumps(request, ensure_ascii=False) + "\n"

    proc = subprocess.Popen(
        [binary, "agent-rpc"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=None,  # Inherit - skillbox stderr to terminal (avoids executor deadlock debug)
        text=True,
        bufsize=1,
        env=env,
        cwd=workspace or os.getcwd(),
    )

    if proc.stdin is None or proc.stdout is None:
        raise RuntimeError("Failed to start skillbox agent-rpc")

    proc.stdin.write(request_line)
    proc.stdin.flush()

    response_text: List[str] = []
    while True:
        line = proc.stdout.readline()
        if not line:
            break
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        event = msg.get("event")
        data = msg.get("data") or {}
        if event == "text":
            chunk = data.get("text", "")
            if chunk and stream_callback:
                stream_callback(chunk)
            if chunk:
                response_text.append(chunk)
        elif event == "text_chunk":
            chunk = data.get("text", "")
            if chunk and stream_callback:
                stream_callback(chunk)
            if chunk:
                response_text.append(chunk)
        elif event == "confirmation_request":
            prompt = data.get("prompt", "")
            approved = False
            if confirmation_callback:
                approved = confirmation_callback(prompt, "")
            confirm_msg = {"method": "confirm", "params": {"approved": approved}}
            proc.stdin.write(json.dumps(confirm_msg, ensure_ascii=False) + "\n")
            proc.stdin.flush()
        elif event == "done":
            return data.get("response", "".join(response_text))
        elif event == "error":
            raise RuntimeError(data.get("message", "Unknown agent error"))

    proc.wait()
    return "".join(response_text)


def agent_chat_stream(
    message: str,
    *,
    session_key: str = "default",
    skill_dirs: Optional[List[str]] = None,
    workspace: Optional[str] = None,
    model: Optional[str] = None,
    api_base: Optional[str] = None,
    api_key: Optional[str] = None,
    max_iterations: Optional[int] = None,
    enable_task_planning: Optional[bool] = None,
    confirmation_callback: Optional[Callable[[str, str], bool]] = None,
) -> Generator[Dict[str, Any], None, None]:
    """
    Run agent chat and yield events (text, tool_call, tool_result, done, error).
    """
    binary = _get_binary()
    env = os.environ.copy()
    _prepare_agent_env(env, api_key, api_base, model, confirmation_callback)

    params: Dict[str, Any] = {
        "message": message,
        "session_key": session_key,
    }
    if skill_dirs:
        params["skill_dirs"] = skill_dirs
    if workspace:
        params["config"] = params.get("config", {})
        params["config"]["workspace"] = workspace
    if model:
        params.setdefault("config", {})["model"] = model
    if api_base:
        params.setdefault("config", {})["api_base"] = api_base
    if api_key:
        params.setdefault("config", {})["api_key"] = api_key
    if max_iterations is not None:
        params.setdefault("config", {})["max_iterations"] = max_iterations
    if enable_task_planning is not None:
        params.setdefault("config", {})["enable_task_planning"] = enable_task_planning

    request = {"method": "agent_chat", "params": params}
    request_line = json.dumps(request, ensure_ascii=False) + "\n"

    proc = subprocess.Popen(
        [binary, "agent-rpc"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=None,  # Inherit - skillbox stderr to terminal
        text=True,
        bufsize=1,
        env=env,
        cwd=workspace or os.getcwd(),
    )

    if proc.stdin is None or proc.stdout is None:
        raise RuntimeError("Failed to start skillbox agent-rpc")

    proc.stdin.write(request_line)
    proc.stdin.flush()

    while True:
        line = proc.stdout.readline()
        if not line:
            break
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        event = msg.get("event")
        data = msg.get("data") or {}
        if event == "confirmation_request":
            prompt = data.get("prompt", "")
            approved = False
            if confirmation_callback:
                approved = confirmation_callback(prompt, "")
            confirm_msg = {"method": "confirm", "params": {"approved": approved}}
            proc.stdin.write(json.dumps(confirm_msg, ensure_ascii=False) + "\n")
            proc.stdin.flush()
        yield msg
        if event in ("done", "error"):
            break
