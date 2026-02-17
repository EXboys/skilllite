"""
IPC client: JSON-RPC over stdio to skilllite serve --stdio.

When SKILLBOX_USE_IPC=1, api.execute_code and benchmark use this instead of subprocess.
"""

import atexit
import json
import os
import subprocess
import threading
from typing import Any, Dict, Optional

from .binary import get_binary

_lock = threading.Lock()
_client: Optional["IPCClient"] = None


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
    One request per line over stdin/stdout.
    """

    def __init__(self, binary: str, cwd: Optional[str] = None):
        self.binary = binary
        self.cwd = cwd or os.getcwd()
        self._process: Optional[subprocess.Popen] = None
        self._request_id = 0
        self._lock = threading.Lock()

    def start(self) -> None:
        """Start the daemon process."""
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

    def close(self) -> None:
        """Terminate the daemon."""
        if self._process and self._process.poll() is None:
            self._process.terminate()
            try:
                self._process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self._process.kill()
            self._process = None

    def _request(self, method: str, params: Dict[str, Any], timeout: float = 60) -> Dict[str, Any]:
        """Send JSON-RPC request, return result or raise on error."""
        if not self._process or self._process.poll() is not None:
            raise RuntimeError("IPC daemon not running")
        with self._lock:
            self._request_id += 1
            req_id = self._request_id
        req = {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
        line = json.dumps(req) + "\n"
        self._process.stdin.write(line)
        self._process.stdin.flush()
        while True:
            out = self._process.stdout.readline()
            if not out:
                raise RuntimeError("IPC daemon closed stdout")
            out = out.rstrip("\r\n")
            if not out:
                continue
            try:
                resp = json.loads(out)
                if isinstance(resp, dict) and ("result" in resp or "error" in resp):
                    break
            except json.JSONDecodeError:
                continue  # Skip log lines (tracing to stdout)
        if "error" in resp:
            raise RuntimeError(resp["error"].get("message", str(resp["error"])))
        return resp.get("result", {})

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
