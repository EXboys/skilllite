"""
MCP server command for skilllite CLI.

Thin wrapper: forwards to skillbox mcp (Rust implementation).
"""

import argparse
import os
import os
import subprocess
import sys
from pathlib import Path

from ..sandbox.core import ensure_installed


def cmd_mcp_server(args: argparse.Namespace) -> int:
    """Start MCP server — forwards to skillbox mcp."""
    skills_dir = getattr(args, "skills_dir", None) or os.environ.get("SKILLLITE_SKILLS_DIR", ".skills")
    binary = ensure_installed()
    cmd = [binary, "mcp", "--skills-dir", skills_dir]
    return subprocess.run(
        cmd,
        env=os.environ,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    ).returncode
