"""
CLI commands — forward to skilllite binary (CLI 全转发).

Consolidates: add, quickstart, list, remove, show, reindex, init-cursor, init-opencode.
"""

import argparse
import os
import subprocess
import sys
from typing import List, Optional


def forward_to_binary(
    cmd: List[str],
    *,
    cwd: Optional[str] = None,
    env: Optional[dict] = None,
    ensure_binary: bool = True,
) -> int:
    """Run skilllite binary with args. Returns exit code."""
    from ..sandbox.core import find_binary, ensure_installed

    binary = ensure_installed() if ensure_binary else find_binary()
    if not binary:
        print("skilllite binary not found. Run `skilllite install` first.", file=sys.stderr)
        return 1

    full_cmd = [str(binary)] + [str(x) for x in cmd]
    run_env = os.environ.copy()
    if env:
        run_env.update(env)

    try:
        result = subprocess.run(
            full_cmd, cwd=cwd, env=run_env,
            stdin=sys.stdin, stdout=sys.stdout, stderr=sys.stderr,
        )
        return result.returncode
    except FileNotFoundError:
        print("skilllite binary not found. Run `skilllite install` first.", file=sys.stderr)
        return 1


# --- Forward commands ---

def cmd_add(args: argparse.Namespace) -> int:
    cmd = ["add", args.source, "-s", args.skills_dir or ".skills"]
    if getattr(args, "force", False): cmd.append("-f")
    if getattr(args, "list", False): cmd.append("-l")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_quickstart(args: argparse.Namespace) -> int:
    cmd = ["quickstart", "-s", getattr(args, "skills_dir", ".skills") or ".skills"]
    env = os.environ.copy()
    if getattr(args, "skills_repo", None):
        env["SKILLLITE_SKILLS_REPO"] = args.skills_repo
    return forward_to_binary(cmd, env=env, ensure_binary=True)


def cmd_list(args: argparse.Namespace) -> int:
    cmd = ["list", "-s", args.skills_dir or ".skills"]
    if getattr(args, "json", False): cmd.append("--json")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_remove(args: argparse.Namespace) -> int:
    cmd = ["remove", args.skill_name, "-s", args.skills_dir or ".skills"]
    if getattr(args, "force", False): cmd.append("-f")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_show(args: argparse.Namespace) -> int:
    cmd = ["show", args.skill_name, "-s", args.skills_dir or ".skills"]
    if getattr(args, "json", False): cmd.append("--json")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_reindex(args: argparse.Namespace) -> int:
    cmd = ["reindex", "-s", args.skills_dir or ".skills"]
    if getattr(args, "verbose", False): cmd.append("-v")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_init_cursor(args: argparse.Namespace) -> int:
    cmd = ["init-cursor", "-s", args.skills_dir or "./.skills"]
    if getattr(args, "project_dir", None): cmd.extend(["-p", args.project_dir])
    if getattr(args, "global_mode", False): cmd.append("-g")
    if getattr(args, "force", False): cmd.append("-f")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_init_opencode(args: argparse.Namespace) -> int:
    cmd = ["init-opencode", "-s", args.skills_dir or "./.skills"]
    if getattr(args, "project_dir", None): cmd.extend(["-p", args.project_dir])
    if getattr(args, "force", False): cmd.append("-f")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_chat(args: argparse.Namespace) -> int:
    from ..quick import load_env
    from pathlib import Path
    load_env()
    if not (os.environ.get("OPENAI_API_KEY") or os.environ.get("API_KEY")):
        print("Error: API_KEY not set. Set in .env or environment.", file=sys.stderr)
        return 1
    workspace = getattr(args, "workspace", None) or str(Path.home() / ".skilllite" / "chat")
    session = getattr(args, "session", "main") or "main"
    skills_dir = getattr(args, "skills_dir", None) or ".skills"
    cmd = ["chat", "--workspace", workspace, "--session", session, "-s", skills_dir]
    if not getattr(args, "quiet", False): cmd.append("--verbose")
    return forward_to_binary(cmd, ensure_binary=True)


def cmd_mcp_server(args: argparse.Namespace) -> int:
    skills_dir = getattr(args, "skills_dir", None) or os.environ.get("SKILLLITE_SKILLS_DIR", ".skills")
    return forward_to_binary(["mcp", "--skills-dir", skills_dir], ensure_binary=True)
