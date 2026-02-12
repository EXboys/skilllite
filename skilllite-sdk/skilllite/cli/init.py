"""
Init command for skilllite CLI.

Provides the ``skilllite init`` command to:
1. Download and install the skillbox binary for the current platform
2. Initialize a .skills directory with a hello-world example skill
3. Scan existing skills and install their dependencies (with environment isolation)

Dependency resolution strategy (in order):
1. Read cached results from ``.skilllite.lock`` (fast, deterministic)
2. Use LLM inference + PyPI/npm registry validation (cold path, ``skilllite init``)
3. Fallback to hardcoded whitelist matching (offline/no-LLM fallback)

Module structure:
- init_binary: binary installation step
- init_deps: lock file, package resolution, venv, security audit
- init_skills: skill templates and directory setup
"""

import argparse
import os
import sys
from pathlib import Path
from typing import Any, Dict, List

from .init_binary import run_binary_step
from .init_deps import scan_and_install_deps, run_dependency_audits
from .init_skills import create_skills_directory


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

        # -- Step 2: .skills directory & example skills -------------------
        force = getattr(args, "force", False)
        created_files: List[str] = create_skills_directory(
            skills_dir, skills_dir_rel, force
        )

        # -- Step 3: Scan & install dependencies ---------------------------
        allow_unknown = getattr(args, "allow_unknown_packages", False) or (
            os.environ.get("SKILLLITE_ALLOW_UNKNOWN_PACKAGES", "").lower()
            in ("1", "true", "yes")
        )
        dep_results: List[Dict[str, Any]] = []
        if not getattr(args, "skip_deps", False):
            print()
            print("\U0001f4e6 Scanning skills and installing dependencies...")
            dep_results = scan_and_install_deps(
                skills_dir, force=force, allow_unknown_packages=allow_unknown
            )

            if not dep_results:
                print("   (no skills found)")
            else:
                for r in dep_results:
                    pkgs = r.get("packages", [])
                    pkg_str = ", ".join(pkgs) if pkgs else "none"
                    status = r.get("status", "unknown")
                    lang = r.get("language", "")
                    resolver = r.get("resolver", "")
                    lang_tag = f" [{lang}]" if lang else ""
                    resolver_tag = f" (via {resolver})" if resolver and resolver != "none" else ""
                    if status.startswith("ok"):
                        print(f"   \u2713 {r['name']}{lang_tag}: {pkg_str}{resolver_tag} \u2014 {status}")
                    else:
                        print(f"   \u2717 {r['name']}{lang_tag}: {pkg_str}{resolver_tag} \u2014 {status}")
                        if r.get("error"):
                            print(f"      {r['error']}")

            dep_errors = [r for r in dep_results if r.get("status") == "error"]
            if dep_errors:
                print()
                print("Error: Some skills failed. Fix the issues above or run with --allow-unknown-packages.")
                return 1
        else:
            print()
            print("\u23ed Skipping dependency installation (--skip-deps)")

        # -- Step 3b: Dependency security audit ----------------------------
        if dep_results and not getattr(args, "skip_audit", False):
            print()
            print("\U0001f512 Scanning dependencies for known vulnerabilities...")
            audit_ok, audit_lines = run_dependency_audits(
                dep_results,
                strict=getattr(args, "strict", False),
                skip_audit=False,
            )
            for line in audit_lines:
                print(line)
            if not audit_ok:
                print()
                print("Error: Dependency audit found vulnerabilities. Fix them or run with --skip-audit.")
                return 1

        # -- Step 4: Summary -----------------------------------------------
        print()
        print("=" * 50)
        print("\U0001f389 SkillLite project initialized successfully!")
        print()
        if created_files:
            print("Created files:")
            for f in created_files:
                print(f"  \u2022 {f}")
            print()
        print("Next steps:")
        print("  \u2022 Add skills to the .skills/ directory")
        print("  \u2022 Run `skilllite status` to check installation")
        print("  \u2022 Run `skilllite init` again after adding new skills to install their deps")
        print("=" * 50)

        return 0

    except Exception as e:
        import traceback
        print(f"Error: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1
