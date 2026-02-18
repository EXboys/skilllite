"""
IPC client: JSON-RPC over stdio to skilllite serve --stdio.

When SKILLBOX_USE_IPC=1, api.execute_code and benchmark use this instead of subprocess.
Supports concurrent requests: batch-sends to daemon so it receives multiple at once.
"""

import atexit
import json
import os
import queue
import subprocess
import threading
import time
from typing import Any, Dict, Optional

from .binary import get_binary

_lock = threading.Lock()
_client: Optional["IPCClient"] = None

# Batch window: wait up to this many seconds for first request, allowing concurrent callers to enqueue
_BATCH_WINDOW_SEC = 0.010
# After first request, brief delay to collect more concurrent requests before sending
_BATCH_DRAIN_DELAY_SEC = 0.005


def _get_client() -> Optional["IPCClient"]:
    """Get or create singleton IPC client. Returns None if IPC disabled or binary missing."""
    global _client
    if os.environ.get("SKILLBOX_USE_IPC") != "1":
        return None
    with _lock:
        if _client is not None:
            return _client
        binary = get_binary()
        if not binary:
            return None
        try:
            _client = IPCClient(binary)
            _client.start()
            atexit.register(_shutdown_client)
            return _client
        except Exception:
            return None


def _shutdown_client() -> None:
    global _client
    with _lock:
        if _client:
            try:
                _client.close()
            except Exception:
                pass
            _client = None


class IPCClient:
    """
    JSON-RPC client to skilllite serve --stdio.
    Batch-sends concurrent requests so daemon receives them together for parallel processing.
    """

    def __init__(self, binary: str, cwd: Optional[str] = None):
        self.binary = binary
        self.cwd = cwd or os.getcwd()
        self._process: Optional[subprocess.Popen] = None
        self._request_id = 0
        self._id_lock = threading.Lock()
        self._send_queue: queue.Queue = queue.Queue()
        self._pending: Dict[int, queue.Queue] = {}
        self._pending_lock = threading.Lock()
        self._reader_thread: Optional[threading.Thread] = None
        self._writer_thread: Optional[threading.Thread] = None
        self._shutdown = threading.Event()

    def start(self) -> None:
        """Start the daemon process, writer thread, and response reader thread."""
        env = dict(os.environ)
        env["SKILLBOX_AUTO_APPROVE"] = "1"
        env["SKILLLITE_QUIET"] = "1"
        env["RUST_LOG"] = "error"  # Suppress INFO/WARN to stdout (IPC channel)
        self._process = subprocess.Popen(
            [self.binary, "serve", "--stdio"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            cwd=self.cwd,
            env=env,
        )
        self._writer_thread = threading.Thread(target=self._write_batch_loop, daemon=True)
        self._writer_thread.start()
        self._reader_thread = threading.Thread(target=self._read_responses, daemon=True)
        self._reader_thread.start()

    def _write_batch_loop(self) -> None:
        """Dedicated writer: collect requests in a short window, send batch so daemon gets many at once."""
        if not self._process or not self._process.stdin:
            return
        try:
            while not self._shutdown.is_set():
                try:
                    first = self._send_queue.get(timeout=_BATCH_WINDOW_SEC)
                except queue.Empty:
                    continue
                if first is None:
                    return
                batch = [first]
                time.sleep(_BATCH_DRAIN_DELAY_SEC)
                while True:
                    try:
                        item = self._send_queue.get_nowait()
                        if item is None:
                            self._send_queue.put(None)
                            return
                        batch.append(item)
                    except queue.Empty:
                        break
                self._process.stdin.write("".join(batch))
                self._process.stdin.flush()
        except (ValueError, OSError):
            pass

    def _read_responses(self) -> None:
        """Background thread: read JSON-RPC responses and route to waiting callers."""
        if not self._process or not self._process.stdout:
            return
        try:
            for line in self._process.stdout:
                if self._shutdown.is_set():
                    break
                line = line.rstrip("\r\n")
                if not line:
                    continue
                try:
                    resp = json.loads(line)
                    if not isinstance(resp, dict) or ("result" not in resp and "error" not in resp):
                        continue
                    req_id = resp.get("id")
                    if req_id is None:
                        continue
                    with self._pending_lock:
                        q = self._pending.pop(req_id, None)
                    if q is not None:
                        q.put(resp)
                except json.JSONDecodeError:
                    continue
        except (ValueError, OSError):
            pass
        # On exit, wake any remaining waiters
        with self._pending_lock:
            for q in self._pending.values():
                try:
                    q.put(None)
                except Exception:
                    pass
            self._pending.clear()

    def close(self) -> None:
        """Terminate the daemon."""
        self._shutdown.set()
        try:
            self._send_queue.put(None)
        except Exception:
            pass
        if self._writer_thread and self._writer_thread.is_alive():
            self._writer_thread.join(timeout=0.5)
        if self._process and self._process.poll() is None:
            self._process.terminate()
            try:
                self._process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self._process.kill()
            self._process = None
        if self._reader_thread and self._reader_thread.is_alive():
            self._reader_thread.join(timeout=1)

    def _request(self, method: str, params: Dict[str, Any], timeout: float = 60) -> Dict[str, Any]:
        """Send JSON-RPC request, return result or raise on error. Safe for concurrent calls."""
        if not self._process or self._process.poll() is not None:
            raise RuntimeError("IPC daemon not running")
        with self._id_lock:
            self._request_id += 1
            req_id = self._request_id
        resp_queue: queue.Queue = queue.Queue()
        with self._pending_lock:
            self._pending[req_id] = resp_queue
        try:
            req = {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
            line = json.dumps(req) + "\n"
            self._send_queue.put(line)
            resp = resp_queue.get(timeout=timeout)
            if resp is None:
                raise RuntimeError("IPC daemon closed")
            if "error" in resp:
                raise RuntimeError(resp["error"].get("message", str(resp["error"])))
            return resp.get("result", {})
        finally:
            with self._pending_lock:
                self._pending.pop(req_id, None)

    def run(
        self,
        skill_dir: str,
        input_json: str,
        *,
        sandbox_level: int = 3,
        allow_network: bool = False,
    ) -> Dict[str, Any]:
        """Run a skill. Returns {output, exit_code}."""
        return self._request(
            "run",
            {
                "skill_dir": skill_dir,
                "input_json": input_json,
                "allow_network": allow_network,
                "sandbox_level": sandbox_level,
            },
        )

    def exec(
        self,
        skill_dir: str,
        script_path: str,
        input_json: str = "{}",
        *,
        sandbox_level: int = 3,
        allow_network: bool = False,
    ) -> Dict[str, Any]:
        """Execute a script. Returns {output, exit_code}."""
        return self._request(
            "exec",
            {
                "skill_dir": skill_dir,
                "script_path": script_path,
                "input_json": input_json,
                "allow_network": allow_network,
                "sandbox_level": sandbox_level,
            },
        )
