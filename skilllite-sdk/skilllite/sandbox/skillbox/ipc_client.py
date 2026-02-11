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

    def _get_client(self, timeout: Optional[float] = None) -> SkillboxIPCClient:
        if self._closed:
            raise RuntimeError("IPC client pool is closed")
        try:
            return self._available.get(block=True, timeout=timeout or 60)
        except Empty:
            raise RuntimeError("IPC pool exhausted (all daemons busy); consider increasing SKILLBOX_IPC_POOL_SIZE")

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

    def close(self) -> None:
        self._closed = True
        for client in self._clients:
            try:
                client.close()
            except Exception:
                pass
