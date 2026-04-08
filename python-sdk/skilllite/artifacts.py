"""
Run-scoped artifact HTTP client (OpenAPI v1).

Talks to ``skilllite artifact-serve`` (with ``SKILLLITE_ARTIFACT_SERVE_ALLOW=1``) or any OpenAPI-compatible server. Uses only the Python
standard library (``urllib``) — no extra pip dependencies.

Typical user flow (e.g. after a model/tool produced a large blob):

1. Choose a stable ``run_id`` for the conversation or job (no ``/`` or ``..``).
2. ``artifact_put(base_url, run_id, "outputs/summary.json", data)``.
3. Another process or host ``artifact_get(...)`` to retrieve the same bytes.
"""

from __future__ import annotations

import urllib.error
import urllib.parse
import urllib.request
from typing import cast


class ArtifactHttpError(Exception):
    """HTTP error from the artifact API (non-404 on GET, any failure on PUT)."""

    def __init__(self, message: str, status: int | None = None) -> None:
        super().__init__(message)
        self.status = status


def _auth_headers(bearer_token: str | None) -> dict[str, str]:
    if bearer_token:
        return {"Authorization": f"Bearer {bearer_token}"}
    return {}


def _artifact_url(base_url: str, run_id: str, key: str) -> str:
    root = base_url.rstrip("/")
    # run_id is restricted by the server; still encode for safe URLs.
    rid = urllib.parse.quote(run_id, safe="")
    q = urllib.parse.urlencode({"key": key})
    return f"{root}/v1/runs/{rid}/artifacts?{q}"


def artifact_put(
    base_url: str,
    run_id: str,
    key: str,
    data: bytes,
    *,
    bearer_token: str | None = None,
    timeout: float = 120.0,
) -> None:
    """
    Store bytes for (run_id, key). Overwrites if the key already exists.

    Raises:
        ArtifactHttpError: on HTTP failure.
        OSError: on network-level errors.
    """
    url = _artifact_url(base_url, run_id, key)
    headers = {
        "Content-Type": "application/octet-stream",
        **_auth_headers(bearer_token),
    }
    req = urllib.request.Request(url, data=data, method="PUT", headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            _ = resp.read()
    except urllib.error.HTTPError as e:
        if e.code in (200, 201, 204):
            return
        body = e.read().decode("utf-8", errors="replace")[:500]
        raise ArtifactHttpError(
            f"artifact PUT failed: HTTP {e.code} {body}",
            status=e.code,
        ) from e


def artifact_get(
    base_url: str,
    run_id: str,
    key: str,
    *,
    bearer_token: str | None = None,
    timeout: float = 120.0,
) -> bytes | None:
    """
    Fetch bytes for (run_id, key).

    Returns:
        Payload bytes, or ``None`` if the server responds with 404 (missing key).

    Raises:
        ArtifactHttpError: on other HTTP errors.
        OSError: on network-level errors.
    """
    url = _artifact_url(base_url, run_id, key)
    headers = _auth_headers(bearer_token)
    req = urllib.request.Request(url, method="GET", headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return cast(bytes, resp.read())
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        body = e.read().decode("utf-8", errors="replace")[:500]
        raise ArtifactHttpError(
            f"artifact GET failed: HTTP {e.code} {body}",
            status=e.code,
        ) from e


def parse_listen_line(line: str) -> str:
    """
    Parse ``SKILLLITE_ARTIFACT_HTTP_ADDR=host:port`` from ``skilllite artifact-serve`` stdout.

    Returns:
        Base URL such as ``http://127.0.0.1:54321``.
    """
    line = line.strip()
    prefix = "SKILLLITE_ARTIFACT_HTTP_ADDR="
    if not line.startswith(prefix):
        raise ValueError(f"expected prefix {prefix!r}, got {line!r}")
    addr = line[len(prefix) :].strip()
    if not addr:
        raise ValueError("empty address")
    # If addr is already a URL, accept it; else assume host:port.
    if addr.startswith("http://") or addr.startswith("https://"):
        return addr.rstrip("/")
    return f"http://{addr}"
