"""IPC client behavior tests.

Uses mocks to avoid spawning real skilllite daemon.
"""

import os
from unittest.mock import MagicMock, patch

from skilllite import ipc


def test_get_client_returns_none_when_ipc_disabled() -> None:
    """_get_client returns None when SKILLLITE_USE_IPC is not '1'."""
    with patch.dict(os.environ, {"SKILLLITE_USE_IPC": "0", "SKILLBOX_USE_IPC": "0"}):
        ipc._client = None
        client = ipc._get_client()
    assert client is None


def test_get_client_returns_none_when_binary_missing() -> None:
    """_get_client returns None when binary not found and IPC enabled."""
    with (
        patch.dict(os.environ, {"SKILLLITE_USE_IPC": "1"}, clear=False),
        patch("skilllite.ipc.get_binary", return_value=None),
    ):
        ipc._client = None
        client = ipc._get_client()
    assert client is None


def test_ipc_client_init_sets_attributes() -> None:
    """IPCClient.__init__ sets binary and cwd correctly."""
    client = ipc.IPCClient("/usr/bin/skilllite", cwd="/tmp")
    assert client.binary == "/usr/bin/skilllite"
    assert client.cwd == "/tmp"


def test_ipc_client_init_uses_cwd_default() -> None:
    """IPCClient.__init__ uses os.getcwd() when cwd is None."""
    with patch("skilllite.ipc.os.getcwd", return_value="/home/user"):
        client = ipc.IPCClient("/bin/skilllite", cwd=None)
    assert client.cwd == "/home/user"


def test_ipc_client_exec_passes_params() -> None:
    """IPCClient.exec forwards params to _request correctly."""
    client = ipc.IPCClient("/fake/skilllite")
    client._request = MagicMock(return_value={"output": "ok", "exit_code": 0})

    result = client.exec("/skill/dir", "main.py", "{}", sandbox_level=2)

    client._request.assert_called_once()
    call_args = client._request.call_args
    assert call_args[0][0] == "exec"
    params = call_args[0][1]
    assert params["skill_dir"] == "/skill/dir"
    assert params["script_path"] == "main.py"
    assert params["input_json"] == "{}"
    assert params["sandbox_level"] == 2
    assert result["output"] == "ok"
    assert result["exit_code"] == 0


def test_ipc_client_run_passes_params() -> None:
    """IPCClient.run forwards params to _request correctly."""
    client = ipc.IPCClient("/fake/skilllite")
    client._request = MagicMock(return_value={"output": "done", "exit_code": 0})

    result = client.run("/skill/dir", '{"x":1}', sandbox_level=3, allow_network=True)

    client._request.assert_called_once()
    call_args = client._request.call_args
    assert call_args[0][0] == "run"
    params = call_args[0][1]
    assert params["skill_dir"] == "/skill/dir"
    assert params["input_json"] == '{"x":1}'
    assert params["sandbox_level"] == 3
    assert params["allow_network"] is True
    assert result["output"] == "done"
