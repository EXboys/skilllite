"""
Init command — binary step + forward to skilllite init.
"""

import argparse
import os
import sys
from pathlib import Path

from ..sandbox.core import install as install_binary, is_installed, get_installed_version
from .commands import forward_to_binary


def _run_binary_step(args: argparse.Namespace) -> None:
    """Install binary if needed (skip if --skip-binary)."""
    if getattr(args, "skip_binary", False):
        print("\u23ed Skipping binary installation (--skip-binary)")
        return
    if is_installed():
        print(f"\u2713 skillbox binary already installed (v{get_installed_version()})")
    else:
        print("\u2b07 Installing skillbox binary...")
        install_binary(show_progress=True)


def cmd_init(args: argparse.Namespace) -> int:
    """Execute ``skilllite init``."""
    try:
        project_dir = Path(args.project_dir or os.getcwd())
        skills_dir_rel = args.skills_dir or ".skills"
        skills_dir_clean = skills_dir_rel[2:] if skills_dir_rel.startswith("./") else skills_dir_rel

        print("\U0001f680 Initializing SkillLite project...")
        print(f"   Project directory: {project_dir}")
        print(f"   Skills directory:  {project_dir / skills_dir_clean}\n")

        _run_binary_step(args)

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

        return forward_to_binary(cmd, cwd=str(project_dir), env=env, ensure_binary=True)
    except Exception as e:
        import traceback
        print(f"Error: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1
