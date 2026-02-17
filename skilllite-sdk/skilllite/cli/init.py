"""
Init command for skilllite CLI.

Provides the ``skilllite init`` command to:
1. Download and install the skillbox binary for the current platform
2. Delegate steps 2–4 to ``skillbox init`` (.skills dir, deps, audit, summary)

Phase 4.1: Steps 2–4 are delegated to skillbox init. Python retains only run_binary_step.
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

from .init_binary import run_binary_step


def run_skillbox_init_for_deps(
    skills_dir: Path,
    *,
    skip_audit: bool = False,
    skip_deps: bool = False,
) -> int:
    """Run skillbox init to install dependencies (Phase 4.2 helper).

    Used by add.py and quickstart.py to delegate dependency installation.
    Returns exit code (0 = success).
    """
    from ..sandbox.skillbox import find_binary

    binary = find_binary()
    if not binary:
        return 1

    project_dir = skills_dir.parent
    skills_dir_rel = skills_dir.name

    cmd = [str(binary), "init", "-s", skills_dir_rel]
    if skip_deps:
        cmd.append("--skip-deps")
    if skip_audit:
        cmd.append("--skip-audit")

    try:
        result = subprocess.run(
            cmd,
            cwd=str(project_dir),
            check=False,
        )
        return result.returncode
    except FileNotFoundError:
        return 1


def cmd_init(args: argparse.Namespace) -> int:
    """Execute the ``skilllite init`` command."""
    try:
        project_dir = Path(args.project_dir or os.getcwd())
        skills_dir_rel = args.skills_dir or ".skills"
        if skills_dir_rel.startswith("./"):
            skills_dir_clean = skills_dir_rel[2:]
        else:
            skills_dir_clean = skills_dir_rel
        skills_dir = project_dir / skills_dir_clean

        print("\U0001f680 Initializing SkillLite project...")
        print(f"   Project directory: {project_dir}")
        print(f"   Skills directory:  {skills_dir}")
        print()

        # -- Step 1: Binary ------------------------------------------------
        run_binary_step(args)

        # -- Steps 2–4: Delegate to skillbox init --------------------------
        from ..sandbox.skillbox import find_binary

        binary = find_binary()
        if not binary:
            print("Error: skillbox binary not found. Run `skilllite install` first.", file=sys.stderr)
            return 1

        cmd = [str(binary), "init", "-s", skills_dir_clean]
        if getattr(args, "skip_deps", False):
            cmd.append("--skip-deps")
        if getattr(args, "skip_audit", False):
            cmd.append("--skip-audit")
        if getattr(args, "strict", False):
            cmd.append("--strict")
        if getattr(args, "use_llm", False):
            cmd.append("--use-llm")

        # Pass allow_unknown_packages via env (skillbox defaults to true; set for consistency)
        allow_unknown = getattr(args, "allow_unknown_packages", False) or (
            os.environ.get("SKILLLITE_ALLOW_UNKNOWN_PACKAGES", "").lower()
            in ("1", "true", "yes")
        )
        env = os.environ.copy()
        env["SKILLLITE_ALLOW_UNKNOWN_PACKAGES"] = "1" if allow_unknown else "0"

        # Note: --force not yet supported by skillbox init; behavior may differ
        try:
            result = subprocess.run(
                cmd,
                cwd=str(project_dir),
                env=env,
                check=False,
            )
            return result.returncode
        except FileNotFoundError:
            print("Error: skillbox binary not found. Run `skilllite install` first.", file=sys.stderr)
            return 1

    except Exception as e:
        import traceback
        print(f"Error: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1
