"""
Built-in tools for SkillLite SDK.

This module provides commonly needed tools like file operations
that can be used with create_enhanced_agentic_loop.

Security: When workspace_root is set, all file operations and run_command
are restricted to that directory to prevent path traversal (e.g. ../../etc/passwd).
"""

import re
import subprocess
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Union

# 危险命令模式：检测 rm -rf、curl|bash、wget|sh 等
_DANGEROUS_COMMAND_PATTERNS = [
    (re.compile(r"\brm\s+(-[rf]+|-rf|-fr)\b", re.I), "rm -rf 递归强制删除"),
    (re.compile(r"rm\s+-rf\s+/", re.I), "rm -rf / 可破坏系统"),
    (re.compile(r"curl\s+[^\s|]+\s*\|\s*(bash|sh)\s*$", re.I), "curl | bash 管道执行远程脚本"),
    (re.compile(r"wget\s+[^\s|]+\s*\|\s*(bash|sh)\s*$", re.I), "wget | sh 管道执行远程脚本"),
    (re.compile(r":\(\)\s*\{\s*:\|\:&\s*\}\s*;\s*:", re.I), "fork 炸弹"),
    (re.compile(r"chmod\s+[0-7]{3,4}\s+(-R|\s+/)", re.I), "chmod 递归修改系统权限"),
]


def _check_dangerous_command(cmd: str) -> Optional[str]:
    """
    检测命令是否包含危险模式。
    返回: 若危险则返回警告原因，否则返回 None。
    """
    cmd_stripped = cmd.strip()
    for pattern, reason in _DANGEROUS_COMMAND_PATTERNS:
        if pattern.search(cmd_stripped):
            return reason
    return None


def _is_sensitive_write_path(path: Path) -> bool:
    """
    判断路径是否为敏感文件，禁止写入。
    包括: .env, .git/config, *.key
    """
    path = path.resolve()
    parts = [p.lower() for p in path.parts]
    name = path.name.lower()
    # .env
    if name == ".env":
        return True
    # .git/config
    if ".git" in parts and len(parts) >= 2 and path.parent.name == ".git" and name == "config":
        return True
    # *.key
    if name.endswith(".key"):
        return True
    return False


def _resolve_within_workspace(
    path: Union[str, Path],
    workspace_root: Optional[Union[str, Path]],
) -> tuple[Path, Optional[str]]:
    """
    Resolve path and ensure it is under workspace_root.
    Returns (resolved_path, error_msg). error_msg is None if valid.
    """
    if workspace_root is None:
        return Path(path).resolve(), None
    try:
        root = Path(workspace_root).resolve()
        p = Path(path).resolve()
        p.relative_to(root)
        return p, None
    except ValueError:
        return (
            Path(path).resolve(),
            f"Path outside workspace: {path} (workspace: {Path(workspace_root).resolve()})",
        )


def get_builtin_file_tools() -> List[Dict[str, Any]]:
    """
    Get built-in file operation tools.
    
    Returns:
        List of tool definitions in OpenAI-compatible format
    """
    return [
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the content of a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to read (relative to workspace/project root, or absolute within workspace)"
                        }
                    },
                    "required": ["file_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write content to a file. Creates the file if it doesn't exist, overwrites if it does.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to write (relative to workspace/project root)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["file_path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_directory",
                "description": "List files and directories in a given path",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "directory_path": {
                            "type": "string",
                            "description": "Path to the directory to list (relative to workspace/project root)"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Whether to list recursively (default: false)",
                            "default": False
                        }
                    },
                    "required": ["directory_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "file_exists",
                "description": "Check if a file or directory exists",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to check (relative to workspace/project root)"
                        }
                    },
                    "required": ["file_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Execute a shell command. Runs in project workspace. Requires user confirmation before execution. Use for dependency installation, setup steps.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute (e.g. 'pip install playwright && playwright install chromium')"
                        }
                    },
                    "required": ["command"]
                }
            }
        }
    ]


def execute_builtin_file_tool(
    tool_name: str,
    tool_input: Dict[str, Any],
    run_command_confirmation: Optional[Callable[[str, str], bool]] = None,
    workspace_root: Optional[Union[str, Path]] = None,
) -> str:
    """
    Execute a built-in tool (file ops or run_command).

    Args:
        tool_name: Name of the tool to execute
        tool_input: Input parameters for the tool
        run_command_confirmation: For run_command, callback (message, id) -> bool before execution
        workspace_root: When set, restricts all file ops and run_command cwd to this directory.
            Prevents path traversal (e.g. ../../etc/passwd). Default None = no confinement.

    Returns:
        Result of the tool execution as a string
    """
    try:
        if tool_name == "run_command":
            cmd = tool_input.get("command", "")
            if not cmd:
                return "Error: command is required"
            if not run_command_confirmation:
                return "Error: run_command 需要用户确认回调，当前未配置。请使用 SkillRunner(confirmation_callback=...) 传入确认函数。"
            danger_reason = _check_dangerous_command(cmd)
            if danger_reason:
                msg = (
                    f"⚠️ 危险命令检测\n\n"
                    f"检测到可能造成严重损害的模式: {danger_reason}\n\n"
                    f"命令: {cmd}\n\n"
                    f"请仔细核实后再确认执行。"
                )
                confirm_id = "run_command_dangerous"
            else:
                msg = f"即将执行命令:\n  {cmd}\n\n是否确认执行？"
                confirm_id = "run_command"
            if not run_command_confirmation(msg, confirm_id):
                return "用户取消了命令执行"
            cwd = str(Path(workspace_root).resolve()) if workspace_root else None
            try:
                import threading

                proc = subprocess.Popen(
                    cmd,
                    shell=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    bufsize=1,
                    cwd=cwd,
                )
                lines = []

                def _read_and_print():
                    for line in proc.stdout:
                        print(line, end="", flush=True)
                        lines.append(line)

                t = threading.Thread(target=_read_and_print, daemon=True)
                t.start()
                proc.wait(timeout=300)  # 5 分钟，适应 playwright install 等耗时命令
                t.join(timeout=1)
                out = "".join(lines)
                if proc.returncode == 0:
                    return f"命令执行成功 (exit 0):\n{out}" if out else "命令执行成功"
                return f"命令执行失败 (exit {proc.returncode}):\n{out}"
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
                return "Error: 命令执行超时 (300s)"
            except Exception as e:
                return f"Error: {e}"
        elif tool_name == "read_file":
            return _read_file(tool_input["file_path"], workspace_root)
        elif tool_name == "write_file":
            return _write_file(tool_input["file_path"], tool_input["content"], workspace_root)
        elif tool_name == "list_directory":
            recursive = tool_input.get("recursive", False)
            return _list_directory(tool_input["directory_path"], recursive, workspace_root)
        elif tool_name == "file_exists":
            return _file_exists(tool_input["file_path"], workspace_root)
        else:
            raise ValueError(f"Unknown built-in tool: {tool_name}")
    except KeyError as e:
        return f"Error: Missing required parameter: {e}"
    except Exception as e:
        return f"Error executing {tool_name}: {str(e)}"


def _read_file(file_path: str, workspace_root: Optional[Union[str, Path]] = None) -> str:
    """Read file content. Restricted to workspace_root when set."""
    path, err = _resolve_within_workspace(file_path, workspace_root)
    if err:
        return f"Error: {err}"
    if not path.exists():
        return f"Error: File not found: {file_path}"
    if not path.is_file():
        return f"Error: Path is not a file: {file_path}"
    try:
        content = path.read_text(encoding="utf-8")
        return f"Successfully read file: {file_path}\n\nContent:\n{content}"
    except UnicodeDecodeError:
        size = path.stat().st_size
        return f"File {file_path} appears to be binary (size: {size} bytes). Cannot display content."
    except Exception as e:
        return f"Error reading file {file_path}: {str(e)}"


def _write_file(
    file_path: str, content: str, workspace_root: Optional[Union[str, Path]] = None
) -> str:
    """Write content to file. Restricted to workspace_root when set.
    Prohibits writing to sensitive paths: .env, .git/config, *.key
    """
    path, err = _resolve_within_workspace(file_path, workspace_root)
    if err:
        return f"Error: {err}"
    if _is_sensitive_write_path(path):
        return (
            f"Error: 禁止写入敏感文件 {file_path}。"
            "受保护路径包括: .env、.git/config、*.key"
        )
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return f"Successfully wrote to file: {file_path} ({len(content)} characters)"
    except Exception as e:
        return f"Error writing to file {file_path}: {str(e)}"


def _list_directory(
    directory_path: str,
    recursive: bool = False,
    workspace_root: Optional[Union[str, Path]] = None,
) -> str:
    """List directory contents. Restricted to workspace_root when set."""
    path, err = _resolve_within_workspace(directory_path, workspace_root)
    if err:
        return f"Error: {err}"
    if not path.exists():
        return f"Error: Directory not found: {directory_path}"
    if not path.is_dir():
        return f"Error: Path is not a directory: {directory_path}"
    try:
        items = []
        if recursive:
            for item in path.rglob("*"):
                try:
                    item_resolved, item_err = _resolve_within_workspace(item, workspace_root)
                    if item_err:
                        continue
                    rel_path = item_resolved.relative_to(path)
                    item_type = "dir" if item_resolved.is_dir() else "file"
                    items.append(f"{item_type}: {rel_path}")
                except (ValueError, OSError):
                    continue
        else:
            for item in path.iterdir():
                try:
                    item_resolved, item_err = _resolve_within_workspace(item, workspace_root)
                    if item_err:
                        continue
                    item_type = "dir" if item_resolved.is_dir() else "file"
                    items.append(f"{item_type}: {item_resolved.name}")
                except (ValueError, OSError):
                    continue
        if not items:
            return f"Directory is empty: {directory_path}"
        items.sort()
        return f"Contents of {directory_path}:\n" + "\n".join(items)
    except Exception as e:
        return f"Error listing directory {directory_path}: {str(e)}"


def _file_exists(file_path: str, workspace_root: Optional[Union[str, Path]] = None) -> str:
    """Check if file exists. Restricted to workspace_root when set."""
    path, err = _resolve_within_workspace(file_path, workspace_root)
    if err:
        return f"Error: {err}"
    if path.exists():
        if path.is_file():
            size = path.stat().st_size
            return f"File exists: {file_path} (size: {size} bytes)"
        elif path.is_dir():
            return f"Directory exists: {file_path}"
        else:
            return f"Path exists but is neither file nor directory: {file_path}"
    else:
        return f"Path does not exist: {file_path}"


def create_builtin_tool_executor(
    run_command_confirmation: Optional[Callable[[str, str], bool]] = None,
    workspace_root: Optional[Union[str, Path]] = None,
):
    """
    Create an executor function for built-in tools.

    Args:
        run_command_confirmation: For run_command, callback before execution. (message, id) -> bool
        workspace_root: Restrict file ops and run_command cwd to this directory (default: None = no restriction)

    Returns:
        Executor function that can be passed to create_enhanced_agentic_loop
    """
    builtin_tool_names = {"read_file", "write_file", "list_directory", "file_exists", "run_command"}

    def executor(tool_input: Dict[str, Any]) -> str:
        """Execute built-in tools."""
        tool_name = tool_input.get("tool_name")
        if tool_name not in builtin_tool_names:
            raise ValueError(f"Not a built-in tool: {tool_name}")
        return execute_builtin_file_tool(
            tool_name,
            tool_input,
            run_command_confirmation=run_command_confirmation if tool_name == "run_command" else None,
            workspace_root=workspace_root,
        )

    return executor
