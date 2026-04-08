"""Artifact HTTP client: unit helpers + user-scenario integration tests.

Integration tests run ``skilllite artifact-serve`` (default binary; needs ``SKILLLITE_ARTIFACT_SERVE_ALLOW=1`` to bind). Build first::

    cargo build -p skilllite --bin skilllite

Override the executable path with ``SKILLLITE_ARTIFACT_HTTP_SERVE`` (path to ``skilllite``).
"""

from __future__ import annotations

import os
import subprocess
import time
from pathlib import Path

import pytest

from skilllite.artifacts import (
    ArtifactHttpError,
    artifact_get,
    artifact_put,
    parse_listen_line,
)


def test_parse_listen_line_ipv4() -> None:
    line = "SKILLLITE_ARTIFACT_HTTP_ADDR=127.0.0.1:54321\n"
    assert parse_listen_line(line) == "http://127.0.0.1:54321"


def test_parse_listen_line_rejects_bad_prefix() -> None:
    with pytest.raises(ValueError, match="expected prefix"):
        parse_listen_line("PORT=1234")


def _artifact_serve_subprocess_env() -> dict[str, str]:
    """Environment for ``artifact-serve`` subprocess (runtime bind allow)."""
    return {**os.environ, "SKILLLITE_ARTIFACT_SERVE_ALLOW": "1"}


def _artifact_serve_cmd_prefix() -> list[str] | None:
    """Return ``[skilllite_exe, \"artifact-serve\"]`` for subprocess."""
    env = os.environ.get("SKILLLITE_ARTIFACT_HTTP_SERVE")
    if env and os.path.isfile(env):
        return [env, "artifact-serve"]
    root = Path(__file__).resolve().parents[2]
    for name in ("debug", "release"):
        for exe in ("skilllite", "skilllite.exe"):
            p = root / "target" / name / exe
            if p.is_file():
                return [str(p), "artifact-serve"]
    try:
        from skilllite.binary import get_binary

        b = get_binary()
        if b:
            return [b, "artifact-serve"]
    except ImportError:
        pass
    return None


@pytest.fixture
def artifact_server_open(tmp_path: Path):
    """Listening server, no bearer token."""
    cmd = _artifact_serve_cmd_prefix()
    if not cmd:
        pytest.skip("skilllite binary not found; run: cargo build -p skilllite --bin skilllite")
    data = tmp_path / "store"
    data.mkdir()
    proc = subprocess.Popen(
        [*cmd, "--dir", str(data), "--bind", "127.0.0.1:0"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=_artifact_serve_subprocess_env(),
    )
    assert proc.stdout is not None
    line = proc.stdout.readline()
    if not line.startswith("SKILLLITE_ARTIFACT_HTTP_ADDR="):
        proc.kill()
        err = proc.stderr.read() if proc.stderr else ""
        pytest.fail(f"bad stdout from skilllite artifact-serve: {line!r} stderr={err!r}")
    base = parse_listen_line(line)
    # Wait until TCP accepts (brief).
    deadline = time.monotonic() + 10.0
    last_err: OSError | None = None
    while time.monotonic() < deadline:
        try:
            import socket

            host_port = base.replace("http://", "")
            host, _, port_s = host_port.partition(":")
            with socket.create_connection((host, int(port_s)), timeout=1.0):
                break
        except OSError as e:
            last_err = e
            time.sleep(0.05)
    else:
        proc.kill()
        pytest.fail(f"server never accepted connections: {last_err}")
    try:
        yield base
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            proc.kill()


@pytest.fixture
def artifact_server_bearer(tmp_path: Path):
    cmd = _artifact_serve_cmd_prefix()
    if not cmd:
        pytest.skip("skilllite binary not found; run: cargo build -p skilllite --bin skilllite")
    data = tmp_path / "store"
    data.mkdir()
    secret = "integration-test-secret"
    proc = subprocess.Popen(
        [
            *cmd,
            "--dir",
            str(data),
            "--bind",
            "127.0.0.1:0",
            "--token",
            secret,
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=_artifact_serve_subprocess_env(),
    )
    assert proc.stdout is not None
    line = proc.stdout.readline()
    if not line.startswith("SKILLLITE_ARTIFACT_HTTP_ADDR="):
        proc.kill()
        err = proc.stderr.read() if proc.stderr else ""
        pytest.fail(f"bad stdout from skilllite artifact-serve: {line!r} stderr={err!r}")
    base = parse_listen_line(line)
    deadline = time.monotonic() + 10.0
    while time.monotonic() < deadline:
        try:
            import socket

            host_port = base.replace("http://", "")
            host, _, port_s = host_port.partition(":")
            with socket.create_connection((host, int(port_s)), timeout=1.0):
                break
        except OSError:
            time.sleep(0.05)
    else:
        proc.kill()
        pytest.fail("bearer server never accepted connections")
    try:
        yield base, secret
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            proc.kill()


def test_scenario_user_model_output_roundtrip(artifact_server_open: str) -> None:
    """User/agent stores a model-produced payload and reads it back (same run)."""
    base = artifact_server_open
    run_id = "session-chat-001"
    payload = b'{"answer": "ok", "tokens": 42}'
    artifact_put(base, run_id, "model/output.json", payload)
    got = artifact_get(base, run_id, "model/output.json")
    assert got == payload


def test_scenario_same_run_multiple_files(artifact_server_open: str) -> None:
    """One conversation run saves two logical artifacts (e.g. summary + raw)."""
    base = artifact_server_open
    run_id = "session-multi-002"
    artifact_put(base, run_id, "step1/summary.txt", b"short")
    artifact_put(base, run_id, "step1/raw.bin", b"\x00\xff")
    assert artifact_get(base, run_id, "step1/summary.txt") == b"short"
    assert artifact_get(base, run_id, "step1/raw.bin") == b"\x00\xff"


def test_scenario_separate_runs_isolated(artifact_server_open: str) -> None:
    """Two runs with the same key must not see each other's data."""
    base = artifact_server_open
    artifact_put(base, "run-a", "out", b"alpha")
    artifact_put(base, "run-b", "out", b"beta")
    assert artifact_get(base, "run-a", "out") == b"alpha"
    assert artifact_get(base, "run-b", "out") == b"beta"


def test_scenario_missing_artifact_returns_none(artifact_server_open: str) -> None:
    """GET of a key that was never written behaves like 'not found'."""
    base = artifact_server_open
    assert artifact_get(base, "run-x", "never-written") is None


def test_scenario_bearer_denies_without_token(artifact_server_bearer: tuple[str, str]) -> None:
    base, _secret = artifact_server_bearer
    with pytest.raises(ArtifactHttpError) as ei:
        artifact_put(base, "r1", "k", b"x", bearer_token=None)
    assert ei.value.status == 401


def test_scenario_bearer_allows_with_token(artifact_server_bearer: tuple[str, str]) -> None:
    base, secret = artifact_server_bearer
    artifact_put(base, "r1", "k", b"secret-payload", bearer_token=secret)
    got = artifact_get(base, "r1", "k", bearer_token=secret)
    assert got == b"secret-payload"
