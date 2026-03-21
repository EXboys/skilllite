"""API behavior tests (scan_code, execute_code, chat, run_skill).

Uses mocks to avoid requiring the skilllite binary.
"""

import json
from unittest.mock import patch

from skilllite import api


def test_scan_code_binary_not_found() -> None:
    """scan_code returns error structure when binary is missing."""
    with patch("skilllite.binary.get_binary", return_value=None):
        result = api.scan_code("python", "print(1)")
    assert result["is_safe"] is False
    assert "issues" in result
    assert "Binary not found" in str(result.get("error", "")) or any(
        "not found" in str(i.get("message", "")) for i in result.get("issues", [])
    )
    assert result.get("requires_confirmation") is False


def test_scan_code_valid_structure_with_mock_binary() -> None:
    """scan_code parses JSON stdout and returns expected keys."""
    mock_stdout = json.dumps(
        {
            "is_safe": True,
            "issues": [],
            "high_severity_count": 0,
            "scan_id": "abc-123",
        }
    )

    with (
        patch("skilllite.binary.get_binary", return_value="/fake/skilllite"),
        patch("skilllite.api.subprocess.run") as mock_run,
    ):
        mock_run.return_value.stdout = mock_stdout
        mock_run.return_value.stderr = ""
        mock_run.return_value.returncode = 0
        result = api.scan_code("python", "x = 1")
    assert result["is_safe"] is True
    assert result["issues"] == []
    assert result.get("scan_id") == "abc-123"
    assert result.get("requires_confirmation") is False


def test_scan_code_requires_confirmation_when_high_severity() -> None:
    """scan_code sets requires_confirmation when high_severity_count > 0."""
    mock_stdout = json.dumps(
        {
            "is_safe": False,
            "issues": [{"severity": "high", "message": "danger"}],
            "high_severity_count": 1,
            "scan_id": "x",
        }
    )

    with (
        patch("skilllite.binary.get_binary", return_value="/fake/skilllite"),
        patch("skilllite.api.subprocess.run") as mock_run,
    ):
        mock_run.return_value.stdout = mock_stdout
        mock_run.return_value.stderr = ""
        mock_run.return_value.returncode = 0
        result = api.scan_code("python", "os.system('rm -rf /')")
    assert result["requires_confirmation"] is True
    assert result["is_safe"] is False


def test_execute_code_binary_not_found() -> None:
    """execute_code returns error when binary missing and IPC disabled."""
    with (
        patch("skilllite.ipc._get_client", return_value=None),
        patch("skilllite.binary.get_binary", return_value=None),
    ):
        result = api.execute_code("python", "print(1)")
    assert result["success"] is False
    assert "exit_code" in result
    assert result["exit_code"] == 1
    assert "not found" in result.get("text", "")


def test_run_skill_binary_not_found() -> None:
    """run_skill returns error when binary missing."""
    with patch("skilllite.binary.get_binary", return_value=None):
        result = api.run_skill("/fake/skill", "{}")
    assert result["success"] is False
    assert result["exit_code"] == 1
    assert "not found" in result.get("text", result.get("stderr", ""))


def test_chat_binary_not_found() -> None:
    """chat returns error when binary missing."""
    with patch("skilllite.binary.get_binary", return_value=None):
        result = api.chat("hello")
    assert result["success"] is False
    assert result["exit_code"] == 1
    assert "not found" in result.get("stderr", "")


def test_execute_code_ipc_path_returns_success_structure() -> None:
    """execute_code via IPC returns correct keys when client succeeds."""
    mock_client = type("MockClient", (), {})()
    mock_client.exec = lambda *a, **kw: {"output": "ok", "exit_code": 0}

    with (
        patch("skilllite.ipc._get_client", return_value=mock_client),
    ):
        result = api.execute_code("python", "print(42)")
    assert result["success"] is True
    assert result["stdout"] == "ok"
    assert result["exit_code"] == 0
    assert result["text"] == "ok"


def test_execute_code_ipc_path_handles_exception() -> None:
    """execute_code via IPC returns error structure when client raises."""

    def _raise(_skill_dir: str, _script: str, _input_json: str = "{}", **_kw: object) -> None:
        raise RuntimeError("daemon closed")

    mock_client = type("MockClient", (), {})()
    mock_client.exec = _raise

    with (
        patch("skilllite.ipc._get_client", return_value=mock_client),
    ):
        result = api.execute_code("python", "print(1)")
    assert result["success"] is False
    assert result["exit_code"] == 1
    assert "daemon" in result.get("stderr", result.get("text", ""))
