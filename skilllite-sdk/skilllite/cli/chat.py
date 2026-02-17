"""
Chat command for skilllite CLI.

Thin wrapper: forwards to skillbox chat (Rust implementation).
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

from ..quick import load_env
from ..sandbox.skillbox import ensure_installed


def cmd_chat(args: argparse.Namespace) -> int:
    """Run interactive chat — forwards to skillbox chat."""
    load_env()

    api_key = os.environ.get("OPENAI_API_KEY") or os.environ.get("API_KEY")
    if not api_key:
        print("Error: API_KEY not set. Set in .env or environment.", file=sys.stderr)
        return 1

    workspace = getattr(args, "workspace", None) or str(Path.home() / ".skilllite" / "chat")
    session = getattr(args, "session", "main") or "main"
    skills_dir = getattr(args, "skills_dir", None) or ".skills"
    verbose = not getattr(args, "quiet", False)

    binary = ensure_installed()
    cmd = [
        binary,
        "chat",
        "--workspace", workspace,
        "--session", session,
        "-s", skills_dir,
    ]
    if verbose:
        cmd.append("--verbose")

    return subprocess.run(
        cmd,
        env=os.environ,
        stdin=sys.stdin,
        stdout=sys.stdout,
        stderr=sys.stderr,
    ).returncode
