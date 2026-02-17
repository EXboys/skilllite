"""
IPC client for skillbox serve --stdio daemon.

Communicates with the skillbox daemon via JSON-RPC over stdin/stdout.
Default: IPC enabled (avoids cold start). Set SKILLBOX_USE_IPC=0 to force subprocess.

Supports connection pooling for concurrent requests: multiple daemons allow
parallel execution instead of serializing on a single pipe.
"""

import json
import os
import subprocess
import threading
from pathlib import Path
from queue import Queue, Empty
from typing import Any, Dict, List, Optional

# Default pool size for concurrent IPC (matches typical benchmark concurrency)
_DEFAULT_POOL_SIZE = int(os.environ.get("SKILLBOX_IPC_POOL_SIZE", "10"))


def _use_ipc() -> bool:
    """Check if IPC mode is enabled. Default: True. Set SKILLBOX_USE_IPC=0 to disable."""
    val = os.environ.get("SKILLBOX_USE_IPC", "").lower()
    return val not in ("0", "false", "no")


class SkillboxIPCClient:
    """
    Client for skillbox IPC daemon (serve --stdio).
    Spawns and maintains a long-lived daemon process, sends JSON-RPC requests.
    """

    def __init__(
        self,
        binary_path: str,
        cache_dir: Optional[str] = None,
    ):
        self.binary_path = binary_path
        self.cache_dir = cache_dir
        self._proc: Optional[subprocess.Popen] = None
        self._request_id = 0
        self._lock = threading.Lock()

    def _ensure_daemon(self, env: Optional[Dict[str, str]] = None) -> None:
        """Start daemon if not running."""
        with self._lock:
            if self._proc is not None and self._proc.poll() is None:
                return

            if self._proc is not None:
                try:
                    self._proc.terminate()
                except Exception:
                    pass
                self._proc = None

            daemon_env = os.environ.copy()
            daemon_env["SKILLBOX_AUTO_APPROVE"] = "1"
            daemon_env["SKILLBOX_QUIET"] = "1"  # Suppress [INFO] for benchmark
            if env:
                daemon_env.update(env)

            self._proc = subprocess.Popen(
                [self.binary_path, "serve", "--stdio"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                bufsize=1,
                env=daemon_env,
            )

    def _send_request(
        self,
        method: str,
        params: Dict[str, Any],
        timeout: Optional[int] = None,
    ) -> Dict[str, Any]:
        """
        Send JSON-RPC request and return result.
        Raises on error or timeout.
        """
        with self._lock:
            self._request_id += 1
            request_id = self._request_id

        request = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }
        request_line = json.dumps(request, ensure_ascii=False) + "\n"

        self._ensure_daemon()

        if self._proc is None or self._proc.stdin is None or self._proc.stdout is None:
            raise RuntimeError("skillbox daemon not running")

        try:
            self._proc.stdin.write(request_line)
            self._proc.stdin.flush()
        except BrokenPipeError as e:
            self._proc = None
            raise RuntimeError(f"skillbox daemon died: {e}") from e

        try:
            response_line = self._proc.stdout.readline()
        except Exception as e:
            self._proc = None
            raise RuntimeError(f"Failed to read from skillbox daemon: {e}") from e

        if not response_line:
            self._proc = None
            raise RuntimeError("skillbox daemon closed connection")

        response = json.loads(response_line.strip())

        if "error" in response:
            err = response["error"]
            msg = err.get("message", str(err))
            raise RuntimeError(msg)

        if "result" not in response:
            raise RuntimeError("Invalid JSON-RPC response: missing result")

        return response["result"]

    def run(
        self,
        skill_dir: str,
        input_json: str,
        allow_network: bool = False,
        cache_dir: Optional[str] = None,
        timeout: Optional[int] = None,
        max_memory: Optional[int] = None,
        sandbox_level: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute run command via IPC."""
        params = {
            "skill_dir": skill_dir,
            "input_json": input_json,
            "allow_network": allow_network,
            "cache_dir": cache_dir,
            "max_memory": max_memory,
            "timeout": timeout,
        }
        if sandbox_level is not None:
            params["sandbox_level"] = int(sandbox_level)
        return self._send_request("run", params, timeout)

    def exec_cmd(
        self,
        skill_dir: str,
        script_path: str,
        input_json: str,
        args: Optional[str] = None,
        allow_network: bool = False,
        cache_dir: Optional[str] = None,
        timeout: Optional[int] = None,
        max_memory: Optional[int] = None,
        sandbox_level: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute exec command via IPC."""
        params = {
            "skill_dir": skill_dir,
            "script_path": script_path,
            "input_json": input_json,
            "args": args,
            "allow_network": allow_network,
            "cache_dir": cache_dir,
            "max_memory": max_memory,
            "timeout": timeout,
        }
        if sandbox_level is not None:
            params["sandbox_level"] = int(sandbox_level)
        return self._send_request("exec", params, timeout)

    def bash(
        self,
        skill_dir: str,
        command: str,
        timeout: Optional[int] = None,
        cwd: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute bash command for bash-tool skill via IPC."""
        params: Dict[str, Any] = {"skill_dir": skill_dir, "command": command}
        if timeout is not None:
            params["timeout"] = timeout
        if cwd is not None:
            params["cwd"] = cwd
        return self._send_request("bash", params, timeout=timeout or 120)

    # --- Chat feature (session, transcript, memory) ---

    def session_create(
        self,
        session_key: str,
        workspace_path: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Create or get session. Requires skillbox built with executor feature."""
        params = {"session_key": session_key}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        return self._send_request("session_create", params)

    def session_get(
        self,
        session_key: str,
        workspace_path: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Get session info."""
        params = {"session_key": session_key}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        return self._send_request("session_get", params)

    def session_update(
        self,
        session_key: str,
        workspace_path: Optional[str] = None,
        **kwargs: Any,
    ) -> Dict[str, Any]:
        """Update session (e.g. token counts)."""
        params = {"session_key": session_key, **kwargs}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        return self._send_request("session_update", params)

    def transcript_append(
        self,
        session_key: str,
        entry: Dict[str, Any],
        workspace_path: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Append entry to transcript."""
        params = {"session_key": session_key, "entry": entry}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        return self._send_request("transcript_append", params)

    def transcript_read(
        self,
        session_key: str,
        workspace_path: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        """Read transcript entries."""
        params = {"session_key": session_key}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        result = self._send_request("transcript_read", params)
        return result if isinstance(result, list) else []

    def transcript_ensure(
        self,
        session_key: str,
        session_id: str,
        workspace_path: Optional[str] = None,
        cwd: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Ensure transcript has session header."""
        params = {"session_key": session_key, "session_id": session_id}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        if cwd is not None:
            params["cwd"] = cwd
        return self._send_request("transcript_ensure", params)

    def memory_write(
        self,
        rel_path: str,
        content: str,
        workspace_path: Optional[str] = None,
        append: bool = False,
        agent_id: str = "default",
    ) -> Dict[str, Any]:
        """Write to memory file."""
        params = {
            "rel_path": rel_path,
            "content": content,
            "append": append,
            "agent_id": agent_id,
        }
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        return self._send_request("memory_write", params)

    def memory_search(
        self,
        query: str,
        limit: int = 10,
        workspace_path: Optional[str] = None,
        agent_id: str = "default",
    ) -> List[Dict[str, Any]]:
        """Search memory (BM25)."""
        params = {"query": query, "limit": limit, "agent_id": agent_id}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        result = self._send_request("memory_search", params)
        return result if isinstance(result, list) else []

    def token_count(self, text: str) -> Dict[str, Any]:
        """Approximate token count (~4 chars per token)."""
        return self._send_request("token_count", {"text": text})

    def build_skills_context(
        self,
        skills_dir: str,
        mode: str = "progressive",
        skills: Optional[List[str]] = None,
    ) -> str:
        """
        Build skills context string for system prompt. Delegates to skilllite RPC.
        Requires skilllite built with agent feature.
        """
        params: Dict[str, Any] = {"skills_dir": skills_dir, "mode": mode}
        if skills is not None:
            params["skills"] = skills
        result = self._send_request("build_skills_context", params)
        return result.get("context", "")

    def list_tools(
        self,
        skills_dir: str,
        skills: Optional[List[str]] = None,
        format: str = "openai",
    ) -> List[Dict[str, Any]]:
        """
        List tool definitions via skilllite RPC. Returns OpenAI or Claude format.
        Requires skilllite built with agent feature.
        """
        params: Dict[str, Any] = {"skills_dir": skills_dir, "format": format}
        if skills is not None:
            params["skills"] = skills
        result = self._send_request("list_tools", params)
        return result.get("tools", [])

    def list_tools_with_meta(
        self,
        skills_dir: str,
        skills: Optional[List[str]] = None,
        format: str = "openai",
    ) -> Dict[str, Any]:
        """
        List tools with execution metadata (skill_dir, script_path per tool).
        For adapter use: execute via run/exec RPC using tool_meta.
        """
        params: Dict[str, Any] = {"skills_dir": skills_dir, "format": format}
        if skills is not None:
            params["skills"] = skills
        return self._send_request("list_tools", params)

    def plan_textify(self, plan: List[Dict[str, Any]]) -> str:
        """Convert plan (task list) to human-readable text. Requires skillbox with executor feature."""
        result = self._send_request("plan_textify", {"plan": plan})
        return result.get("text", "")

    def plan_write(
        self,
        session_key: str,
        task_id: str,
        task: str,
        steps: List[Dict[str, Any]],
        workspace_path: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Write plan to plans/{session_key}-{date}.json (overwrite). OpenClaw-style."""
        params = {"session_key": session_key, "task_id": task_id, "task": task, "steps": steps}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        return self._send_request("plan_write", params)

    def plan_read(
        self,
        session_key: str,
        workspace_path: Optional[str] = None,
        date: Optional[str] = None,
    ) -> Optional[Dict[str, Any]]:
        """Read plan from plans/{session_key}-{date}.json."""
        params = {"session_key": session_key}
        if workspace_path is not None:
            params["workspace_path"] = workspace_path
        if date is not None:
            params["date"] = date
        result = self._send_request("plan_read", params)
        return result if result is not None else None

    def close(self) -> None:
        """Terminate the daemon process."""
        with self._lock:
            if self._proc is not None:
                try:
                    self._proc.terminate()
                    self._proc.wait(timeout=2)
                except Exception:
                    pass
                self._proc = None


class SkillboxIPCClientPool:
    """
    Connection pool for concurrent IPC requests.
    
    Maintains N daemon processes; each handles one request at a time.
    With pool_size=10 and 10 concurrent callers, all run in parallel.
    """

    def __init__(
        self,
        binary_path: str,
        cache_dir: Optional[str] = None,
        pool_size: Optional[int] = None,
    ):
        self.binary_path = binary_path
        self.cache_dir = cache_dir
        self.pool_size = pool_size or _DEFAULT_POOL_SIZE
        self._clients: List[SkillboxIPCClient] = []
        self._available: Queue[SkillboxIPCClient] = Queue()
        self._closed = False

        for _ in range(self.pool_size):
            client = SkillboxIPCClient(binary_path=binary_path, cache_dir=cache_dir)
            self._clients.append(client)
            self._available.put(client)

    def get_peak_daemon_memory_kb(self) -> float:
        """Return max RSS (KB) across all daemon processes. For benchmark memory reporting."""
        peak_kb = 0.0
        for client in self._clients:
            proc = client._proc
            if proc is not None and proc.poll() is None:
                rss_kb = _get_process_rss_kb(proc.pid)
                if rss_kb > 0:
                    peak_kb = max(peak_kb, rss_kb)
        return peak_kb

    def _get_client(self, timeout: Optional[float] = None) -> SkillboxIPCClient:
        if self._closed:
            raise RuntimeError("IPC client pool is closed")
        try:
            return self._available.get(block=True, timeout=timeout or 60)
        except Empty:
            raise RuntimeError(
                "IPC pool exhausted (all daemons busy); consider increasing SKILLBOX_IPC_POOL_SIZE"
            )

    def _return_client(self, client: SkillboxIPCClient) -> None:
        if not self._closed:
            self._available.put(client)

    def run(
        self,
        skill_dir: str,
        input_json: str,
        allow_network: bool = False,
        cache_dir: Optional[str] = None,
        timeout: Optional[int] = None,
        max_memory: Optional[int] = None,
        sandbox_level: Optional[str] = None,
    ) -> Dict[str, Any]:
        client = self._get_client(timeout)
        try:
            return client.run(
                skill_dir=skill_dir,
                input_json=input_json,
                allow_network=allow_network,
                cache_dir=cache_dir or self.cache_dir,
                timeout=timeout,
                max_memory=max_memory,
                sandbox_level=sandbox_level,
            )
        finally:
            self._return_client(client)

    def exec_cmd(
        self,
        skill_dir: str,
        script_path: str,
        input_json: str,
        args: Optional[str] = None,
        allow_network: bool = False,
        cache_dir: Optional[str] = None,
        timeout: Optional[int] = None,
        max_memory: Optional[int] = None,
        sandbox_level: Optional[str] = None,
    ) -> Dict[str, Any]:
        client = self._get_client(timeout)
        try:
            return client.exec_cmd(
                skill_dir=skill_dir,
                script_path=script_path,
                input_json=input_json,
                args=args,
                allow_network=allow_network,
                cache_dir=cache_dir or self.cache_dir,
                timeout=timeout,
                max_memory=max_memory,
                sandbox_level=sandbox_level,
            )
        finally:
            self._return_client(client)

    def bash(
        self,
        skill_dir: str,
        command: str,
        timeout: Optional[int] = None,
        cwd: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute bash command for bash-tool skill."""
        client = self._get_client(timeout=timeout or 120)
        try:
            return client.bash(
                skill_dir=skill_dir,
                command=command,
                timeout=timeout,
                cwd=cwd,
            )
        finally:
            self._return_client(client)

    def build_skills_context(
        self,
        skills_dir: str,
        mode: str = "progressive",
        skills: Optional[List[str]] = None,
    ) -> str:
        """Build skills context via RPC. Delegates to skilllite build_skills_context."""
        client = self._get_client(timeout=30)
        try:
            return client.build_skills_context(
                skills_dir=skills_dir,
                mode=mode,
                skills=skills,
            )
        finally:
            self._return_client(client)

    def list_tools(
        self,
        skills_dir: str,
        skills: Optional[List[str]] = None,
        format: str = "openai",
    ) -> List[Dict[str, Any]]:
        """List tool definitions via RPC. Delegates to skilllite list_tools."""
        client = self._get_client(timeout=30)
        try:
            return client.list_tools(
                skills_dir=skills_dir,
                skills=skills,
                format=format,
            )
        finally:
            self._return_client(client)

    def list_tools_with_meta(
        self,
        skills_dir: str,
        skills: Optional[List[str]] = None,
        format: str = "openai",
    ) -> Dict[str, Any]:
        """List tools with execution metadata. For adapter use."""
        client = self._get_client(timeout=30)
        try:
            return client.list_tools_with_meta(
                skills_dir=skills_dir,
                skills=skills,
                format=format,
            )
        finally:
            self._return_client(client)

    def close(self) -> None:
        self._closed = True
        for client in self._clients:
            try:
                client.close()
            except Exception:
                pass


def _get_process_rss_kb(pid: int) -> float:
    """Get process RSS in KB. Uses psutil if available, else /proc on Linux."""
    try:
        import psutil
        p = psutil.Process(pid)
        return p.memory_info().rss / 1024
    except ImportError:
        pass
    except (psutil.NoSuchProcess, psutil.AccessDenied, OSError):
        return 0.0
    # Fallback: /proc on Linux
    try:
        with open(f"/proc/{pid}/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return float(line.split()[1])
    except (FileNotFoundError, OSError, ValueError):
        pass
    return 0.0
