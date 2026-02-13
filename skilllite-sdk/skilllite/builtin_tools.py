"""
Built-in tools for SkillLite SDK.

This module provides commonly needed tools like file operations
that can be used with create_enhanced_agentic_loop.

Security: When workspace_root is set, all file operations and run_command
are restricted to that directory to prevent path traversal (e.g. ../../etc/passwd).

When SANDBOX_BUILTIN_TOOLS=1, file operations (read_file, write_file, list_directory, file_exists)
run in a separate subprocess for isolation; run_command stays in main process (needs confirmation).
"""

import http.server
import json
import os
import re
import socketserver
import subprocess
import sys
import threading
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Union

# Dangerous command patterns: detect rm -rf, curl|bash, wget|sh, etc.
_DANGEROUS_COMMAND_PATTERNS = [
    (re.compile(r"\brm\s+(-[rf]+|-rf|-fr)\b", re.I), "rm -rf recursive force delete"),
    (re.compile(r"rm\s+-rf\s+/", re.I), "rm -rf / can destroy system"),
    (re.compile(r"curl\s+[^\s|]+\s*\|\s*(bash|sh)\s*$", re.I), "curl | bash pipe executes remote script"),
    (re.compile(r"wget\s+[^\s|]+\s*\|\s*(bash|sh)\s*$", re.I), "wget | sh pipe executes remote script"),
    (re.compile(r":\(\)\s*\{\s*:\|\:&\s*\}\s*;\s*:", re.I), "fork bomb"),
    (re.compile(r"chmod\s+[0-7]{3,4}\s+(-R|\s+/)", re.I), "chmod recursive system permission change"),
]


def _check_dangerous_command(cmd: str) -> Optional[str]:
    """
    Check if command contains dangerous patterns.
    Returns: warning reason if dangerous, else None.
    """
    cmd_stripped = cmd.strip()
    for pattern, reason in _DANGEROUS_COMMAND_PATTERNS:
        if pattern.search(cmd_stripped):
            return reason
    return None


def _is_sensitive_write_path(path: Path) -> bool:
    """
    Check if path is sensitive file, prohibit write.
    Includes: .env, .git/config, *.key
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


def resolve_output_dir(workspace_root: Union[str, Path]) -> Path:
    """
    Resolve output directory for final results.
    Uses SKILLLITE_OUTPUT_DIR env: absolute path as-is, relative path vs workspace_root.
    Default: workspace_root/output
    """
    root = Path(workspace_root).resolve()
    env_val = os.environ.get("SKILLLITE_OUTPUT_DIR", "").strip()
    if not env_val:
        return root / "output"
    p = Path(env_val)
    if p.is_absolute():
        return p
    return root / p


def _resolve_within_workspace(
    path: Union[str, Path],
    workspace_root: Optional[Union[str, Path]],
) -> tuple[Path, Optional[str]]:
    """
    Resolve path and ensure it is under workspace_root.
    When workspace_root is set, "." and "" resolve to workspace root.
    Returns (resolved_path, error_msg). error_msg is None if valid.
    """
    if workspace_root is None:
        return Path(path).resolve(), None
    root = Path(workspace_root).resolve()
    path_str = str(path).strip()
    if path_str in ("", "."):
        return root, None
    p = Path(path)
    if not p.is_absolute():
        p = root / p
    p = p.resolve()
    try:
        p.relative_to(root)
        return p, None
    except ValueError:
        return (
            p,
            f"Path outside workspace: {path} (workspace: {root})",
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
                "description": "Write content to a file in the project. Use for creating/modifying project files (e.g. .skills/xxx/SKILL.md). For final output (reports, images), prefer write_output.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path relative to workspace root"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write"
                        }
                    },
                    "required": ["file_path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_output",
                "description": "Write final output to the output directory (separate from plan/memory/logs). Use for reports, generated content, images. Path is relative to output dir (e.g. report.md, image.png).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Filename or path relative to output dir (e.g. report.md, results/data.json)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write"
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
        },
        {
            "type": "function",
            "function": {
                "name": "preview_server",
                "description": "Start a local HTTP server to preview HTML files in browser. Serves the given directory at http://127.0.0.1:PORT. Use after writing HTML to output/ with write_output. Server runs in background until process exits.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "directory_path": {
                            "type": "string",
                            "description": "Path relative to workspace (e.g. output, output/preview). If a file path (e.g. output/index.html), serves its parent directory."
                        },
                        "port": {
                            "type": "integer",
                            "description": "Port number (default: 8765). If busy, will try next available.",
                            "default": 8765
                        },
                        "open_browser": {
                            "type": "boolean",
                            "description": "Whether to open browser automatically (default: true)",
                            "default": True
                        }
                    },
                    "required": ["directory_path"]
                }
            }
        }
    ]


def execute_builtin_file_tool(
    tool_name: str,
    tool_input: Dict[str, Any],
    run_command_confirmation: Optional[Callable[[str, str], bool]] = None,
    workspace_root: Optional[Union[str, Path]] = None,
    output_root: Optional[Union[str, Path]] = None,
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
                return "Error: run_command requires confirmation callback. Use SkillRunner(confirmation_callback=...) to pass one."
            danger_reason = _check_dangerous_command(cmd)
            if danger_reason:
                msg = (
                    f"⚠️ Dangerous command detected\n\n"
                    f"Pattern that may cause serious harm: {danger_reason}\n\n"
                    f"Command: {cmd}\n\n"
                    f"Please verify before confirming execution."
                )
                confirm_id = "run_command_dangerous"
            else:
                msg = f"About to execute command:\n  {cmd}\n\nConfirm execution?"
                confirm_id = "run_command"
            if not run_command_confirmation(msg, confirm_id):
                return "User cancelled command execution"
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
                proc.wait(timeout=300)  # 5 min, for playwright install etc.
                t.join(timeout=1)
                out = "".join(lines)
                if proc.returncode == 0:
                    return f"Command succeeded (exit 0):\n{out}" if out else "Command succeeded"
                return f"Command failed (exit {proc.returncode}):\n{out}"
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
                return "Error: Command execution timeout (300s)"
            except Exception as e:
                return f"Error: {e}"
        elif tool_name == "read_file":
            return _read_file(tool_input["file_path"], workspace_root)
        elif tool_name == "write_file":
            return _write_file(tool_input["file_path"], tool_input["content"], workspace_root)
        elif tool_name == "write_output":
            out_root = output_root or (Path(workspace_root).resolve() / "output" if workspace_root else None)
            if not out_root:
                return "Error: output_root not configured"
            return _write_file(tool_input["file_path"], tool_input["content"], out_root)
        elif tool_name == "list_directory":
            recursive = tool_input.get("recursive", False)
            return _list_directory(tool_input["directory_path"], recursive, workspace_root)
        elif tool_name == "file_exists":
            return _file_exists(tool_input["file_path"], workspace_root)
        elif tool_name == "preview_server":
            return _start_preview_server(
                directory_path=tool_input["directory_path"],
                port=tool_input.get("port", 8765),
                open_browser=tool_input.get("open_browser", True),
                workspace_root=workspace_root,
            )
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
            f"Error: Cannot write to sensitive file {file_path}. "
            "Protected paths: .env, .git/config, *.key"
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


def _start_preview_server(
    directory_path: str,
    port: int = 8765,
    open_browser: bool = True,
    workspace_root: Optional[Union[str, Path]] = None,
) -> str:
    """Start a local HTTP server to preview HTML. Binds to 127.0.0.1 only. Runs in daemon thread."""
    path, err = _resolve_within_workspace(directory_path, workspace_root)
    if err:
        return f"Error: {err}"
    if not path.exists():
        return f"Error: Path not found: {directory_path}"
    serve_dir = path.parent if path.is_file() else path

    port = int(port) if port else 8765
    server = None
    used_port = None

    # Handler that serves from serve_dir (Python 3.9+ has directory param; else chdir in thread)
    class _Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            if sys.version_info >= (3, 9):
                kwargs["directory"] = str(serve_dir)
            super().__init__(*args, **kwargs)

    for p in range(port, min(port + 20, 65536)):
        try:
            server = socketserver.TCPServer(("127.0.0.1", p), _Handler)
            used_port = p
            break
        except OSError:
            continue

    if server is None or used_port is None:
        return f"Error: Could not bind to port {port} (tried {port}-{port + 19})"

    def serve():
        if sys.version_info < (3, 9):
            os.chdir(serve_dir)
        server.serve_forever()

    t = threading.Thread(target=serve, daemon=True)
    t.start()

    url = f"http://127.0.0.1:{used_port}"
    if open_browser:
        try:
            import webbrowser
            webbrowser.open(url)
        except Exception:
            pass

    return (
        f"Preview server started at {url}\n\n"
        f"Open in browser: {url}\n"
        f"Serving directory: {serve_dir}\n"
        f"(Server runs in background. Stops when you exit.)"
    )


# File-only tools that can run in sandbox subprocess (no user confirmation needed)
_SANDBOXED_TOOL_NAMES = frozenset({"read_file", "write_file", "write_output", "list_directory", "file_exists"})


def _run_file_tool_in_subprocess(
    tool_name: str,
    tool_input: Dict[str, Any],
    workspace_root: Optional[Union[str, Path]],
    output_root: Optional[Union[str, Path]] = None,
) -> str:
    """
    Execute a file tool (read_file, write_file, write_output, list_directory, file_exists) in subprocess.
    Isolates execution from main process for defense-in-depth.
    """
    req = {
        "tool_name": tool_name,
        "tool_input": tool_input,
        "workspace_root": str(workspace_root) if workspace_root else None,
        "output_root": str(output_root) if output_root else None,
    }
    try:
        proc = subprocess.run(
            [
                sys.executable, "-c",
                _SUBPROCESS_WORKER_SCRIPT,
            ],
            input=json.dumps(req) + "\n",
            capture_output=True,
            text=True,
            timeout=30,
            cwd=str(workspace_root) if workspace_root else None,
        )
        if proc.returncode != 0:
            return f"Error: subprocess failed: {proc.stderr or proc.stdout}"
        data = json.loads(proc.stdout.strip())
        if data.get("ok"):
            return data["result"]
        return f"Error: {data.get('error', 'unknown')}"
    except subprocess.TimeoutExpired:
        return "Error: tool execution timed out (30s)"
    except json.JSONDecodeError as e:
        return f"Error: invalid subprocess output: {e}"
    except Exception as e:
        return f"Error: {e}"


_SUBPROCESS_WORKER_SCRIPT = """
import json
import sys

def run():
    line = sys.stdin.read().strip()
    if not line:
        sys.exit(1)
    req = json.loads(line)
    tool_name = req["tool_name"]
    tool_input = req["tool_input"]
    workspace_root = req.get("workspace_root")
    output_root = req.get("output_root")
    try:
        from skilllite.builtin_tools import (
            _read_file,
            _write_file,
            _list_directory,
            _file_exists,
        )
        if tool_name == "read_file":
            result = _read_file(tool_input["file_path"], workspace_root)
        elif tool_name == "write_file":
            result = _write_file(
                tool_input["file_path"],
                tool_input["content"],
                workspace_root,
            )
        elif tool_name == "write_output":
            root = output_root or (f"{workspace_root}/output" if workspace_root else None)
            result = _write_file(
                tool_input["file_path"],
                tool_input["content"],
                root,
            ) if root else "Error: output_root not configured"
        elif tool_name == "list_directory":
            result = _list_directory(
                tool_input["directory_path"],
                tool_input.get("recursive", False),
                workspace_root,
            )
        elif tool_name == "file_exists":
            result = _file_exists(tool_input["file_path"], workspace_root)
        else:
            result = f"Error: Unsupported tool in sandbox: {tool_name}"
        print(json.dumps({"ok": True, "result": result}))
    except Exception as e:
        print(json.dumps({"ok": False, "error": str(e)}))

if __name__ == "__main__":
    run()
"""


def create_builtin_tool_executor(
    run_command_confirmation: Optional[Callable[[str, str], bool]] = None,
    workspace_root: Optional[Union[str, Path]] = None,
    output_root: Optional[Union[str, Path]] = None,
    use_sandbox: Optional[bool] = None,
):
    """
    Create an executor function for built-in tools.

    Args:
        run_command_confirmation: For run_command, callback before execution. (message, id) -> bool
        workspace_root: Restrict file ops and run_command cwd to this directory (default: None = no restriction)
        output_root: Directory for write_output (final results). Default: workspace_root/output or from SKILLLITE_OUTPUT_DIR
        use_sandbox: When True, run file tools in subprocess for isolation.
            When None, reads from SANDBOX_BUILTIN_TOOLS env (1/true = enabled).

    Returns:
        Executor function that can be passed to create_enhanced_agentic_loop
    """
    if use_sandbox is None:
        use_sandbox = os.environ.get("SANDBOX_BUILTIN_TOOLS", "0").strip().lower() in ("1", "true", "yes")

    builtin_tool_names = {"read_file", "write_file", "write_output", "list_directory", "file_exists", "run_command", "preview_server"}
    resolved_output = Path(output_root).resolve() if output_root else (resolve_output_dir(workspace_root) if workspace_root else None)

    def executor(tool_input: Dict[str, Any]) -> str:
        """Execute built-in tools."""
        tool_name = tool_input.get("tool_name")
        if tool_name not in builtin_tool_names:
            raise ValueError(f"Not a built-in tool: {tool_name}")
        # run_command always runs in main process (needs confirmation callback)
        if tool_name == "run_command":
            return execute_builtin_file_tool(
                tool_name,
                tool_input,
                run_command_confirmation=run_command_confirmation,
                workspace_root=workspace_root,
            )
        if use_sandbox and tool_name in _SANDBOXED_TOOL_NAMES:
            return _run_file_tool_in_subprocess(tool_name, tool_input, workspace_root, resolved_output)
        return execute_builtin_file_tool(
            tool_name,
            tool_input,
            run_command_confirmation=None,
            workspace_root=workspace_root,
            output_root=resolved_output,
        )

    return executor
