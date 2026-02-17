"""
CLI commands — forward to skilllite binary (CLI 全转发).

Consolidates: add, quickstart, list, remove, show, reindex, init-cursor, init-opencode,
init, install, uninstall, status, version.
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
    use_sandbox_binary: bool = False,
) -> int:
    """Run skilllite binary with args. Returns exit code.
    use_sandbox_binary: use find_sandbox_binary() for sandbox-only commands (mcp, add, list, etc).
    """
    from ..sandbox.core import find_binary, find_sandbox_binary, ensure_installed

    if ensure_binary:
        ensure_installed()
    find_fn = find_sandbox_binary if use_sandbox_binary else find_binary
    binary = find_fn()
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
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


def cmd_quickstart(args: argparse.Namespace) -> int:
    cmd = ["quickstart", "-s", getattr(args, "skills_dir", ".skills") or ".skills"]
    env = os.environ.copy()
    if getattr(args, "skills_repo", None):
        env["SKILLLITE_SKILLS_REPO"] = args.skills_repo
    return forward_to_binary(cmd, env=env, ensure_binary=True, use_sandbox_binary=True)


def cmd_list(args: argparse.Namespace) -> int:
    cmd = ["list", "-s", args.skills_dir or ".skills"]
    if getattr(args, "json", False): cmd.append("--json")
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


def cmd_remove(args: argparse.Namespace) -> int:
    cmd = ["remove", args.skill_name, "-s", args.skills_dir or ".skills"]
    if getattr(args, "force", False): cmd.append("-f")
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


def cmd_show(args: argparse.Namespace) -> int:
    cmd = ["show", args.skill_name, "-s", args.skills_dir or ".skills"]
    if getattr(args, "json", False): cmd.append("--json")
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


def cmd_reindex(args: argparse.Namespace) -> int:
    cmd = ["reindex", "-s", args.skills_dir or ".skills"]
    if getattr(args, "verbose", False): cmd.append("-v")
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


def cmd_init_cursor(args: argparse.Namespace) -> int:
    cmd = ["init-cursor", "-s", args.skills_dir or "./.skills"]
    if getattr(args, "project_dir", None): cmd.extend(["-p", args.project_dir])
    if getattr(args, "global_mode", False): cmd.append("-g")
    if getattr(args, "force", False): cmd.append("-f")
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


def cmd_init_opencode(args: argparse.Namespace) -> int:
    cmd = ["init-opencode", "-s", args.skills_dir or "./.skills"]
    if getattr(args, "project_dir", None): cmd.extend(["-p", args.project_dir])
    if getattr(args, "force", False): cmd.append("-f")
    return forward_to_binary(cmd, ensure_binary=True, use_sandbox_binary=True)


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
    return forward_to_binary(["mcp", "--skills-dir", skills_dir], ensure_binary=True, use_sandbox_binary=True)


# --- Binary commands (install/uninstall/status/version) ---

def _print_status() -> None:
    from ..sandbox.core import find_binary, find_sandbox_binary, get_binary_path, is_installed, get_installed_version
    print("SkillLite Installation Status")
    print("=" * 40)
    if is_installed():
        print(f"✓ skillbox is installed (v{get_installed_version()})")
        print(f"  Location: {get_binary_path()}")
    else:
        binary = find_sandbox_binary() or find_binary()
        if binary:
            print(f"✓ skillbox found at: {binary}")
        else:
            print("✗ skillbox is not installed")
            print("  Install with: skilllite install")


def cmd_install(args: argparse.Namespace) -> int:
    from ..sandbox.core import install
    try:
        install(version=args.version, force=args.force, show_progress=not args.quiet)
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_uninstall(args: argparse.Namespace) -> int:
    from ..sandbox.core import uninstall
    try:
        uninstall()
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_status(args: argparse.Namespace) -> int:
    try:
        _print_status()
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_version(args: argparse.Namespace) -> int:
    from .. import __version__
    from ..sandbox.core import BINARY_VERSION, get_platform, get_installed_version
    print(f"skilllite Python SDK: v{__version__}")
    print(f"skillbox binary (bundled): v{BINARY_VERSION}")
    iv = get_installed_version()
    print(f"skillbox binary (installed): v{iv}" if iv else "skillbox binary (installed): not installed")
    try:
        print(f"Platform: {get_platform()}")
    except RuntimeError as e:
        print(f"Platform: {e}")
    return 0


# --- Init command ---

def cmd_init(args: argparse.Namespace) -> int:
    from pathlib import Path
    from ..sandbox.core import install as install_binary, is_installed, get_installed_version
    try:
        project_dir = Path(args.project_dir or os.getcwd())
        skills_dir_rel = args.skills_dir or ".skills"
        skills_dir_clean = skills_dir_rel[2:] if skills_dir_rel.startswith("./") else skills_dir_rel
        print("\U0001f680 Initializing SkillLite project...")
        print(f"   Project directory: {project_dir}")
        print(f"   Skills directory:  {project_dir / skills_dir_clean}\n")
        if not getattr(args, "skip_binary", False):
            if is_installed():
                print(f"\u2713 skillbox binary already installed (v{get_installed_version()})")
            else:
                print("\u2b07 Installing skillbox binary...")
                install_binary(show_progress=True)
        else:
            print("\u23ed Skipping binary installation (--skip-binary)")
        cmd = ["init", "-s", skills_dir_clean]
        if getattr(args, "skip_deps", False): cmd.append("--skip-deps")
        if getattr(args, "skip_audit", False): cmd.append("--skip-audit")
        if getattr(args, "strict", False): cmd.append("--strict")
        if getattr(args, "use_llm", False): cmd.append("--use-llm")
        env = {}
        allow = getattr(args, "allow_unknown_packages", False) or (
            os.environ.get("SKILLLITE_ALLOW_UNKNOWN_PACKAGES", "").lower() in ("1", "true", "yes")
        )
        env["SKILLLITE_ALLOW_UNKNOWN_PACKAGES"] = "1" if allow else "0"
        return forward_to_binary(cmd, cwd=str(project_dir), env=env, ensure_binary=True, use_sandbox_binary=True)
    except Exception as e:
        import traceback
        print(f"Error: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1
