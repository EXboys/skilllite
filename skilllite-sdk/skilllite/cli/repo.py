"""
Repository management commands for skilllite CLI.

Provides ``skilllite list`` and ``skilllite remove`` commands.
"""

import argparse
import os
import shutil
from pathlib import Path
from typing import List

from ..core.metadata import parse_skill_metadata, detect_language


# ---------------------------------------------------------------------------
# cmd_list
# ---------------------------------------------------------------------------

def cmd_list(args: argparse.Namespace) -> int:
    """Execute the ``skilllite list`` command.

    Lists all installed skills in the .skills/ directory.
    """
    skills_dir = Path(args.skills_dir or ".skills")
    if not skills_dir.is_absolute():
        skills_dir = Path(os.getcwd()) / skills_dir

    if not skills_dir.exists():
        print("No skills directory found. Run `skilllite init` first.")
        return 0

    skill_dirs: List[Path] = []
    for child in sorted(skills_dir.iterdir()):
        if child.is_dir() and (child / "SKILL.md").exists():
            skill_dirs.append(child)

    if not skill_dirs:
        print("No skills installed.")
        return 0

    print(f"📋 Installed skills ({len(skill_dirs)}):")
    print()
    for skill_path in skill_dirs:
        try:
            meta = parse_skill_metadata(skill_path)
            name = meta.name or skill_path.name
            desc = meta.description or ""
            lang = detect_language(skill_path, meta)
            version = meta.version or "-"

            lang_tag = f"[{lang}]" if lang and lang != "unknown" else ""
            print(f"  • {name} {lang_tag}")
            if desc:
                print(f"    {desc[:80]}")
            print(f"    version: {version}  path: {skill_path}")
        except Exception as e:
            print(f"  • {skill_path.name}")
            print(f"    ⚠ Could not parse SKILL.md: {e}")
        print()

    return 0


# ---------------------------------------------------------------------------
# cmd_remove
# ---------------------------------------------------------------------------

def cmd_remove(args: argparse.Namespace) -> int:
    """Execute the ``skilllite remove <skill-name>`` command.

    Removes an installed skill from the .skills/ directory.
    """
    skill_name: str = args.skill_name
    skills_dir = Path(args.skills_dir or ".skills")
    if not skills_dir.is_absolute():
        skills_dir = Path(os.getcwd()) / skills_dir

    if not skills_dir.exists():
        print("No skills directory found. Nothing to remove.")
        return 1

    skill_path = skills_dir / skill_name
    if not skill_path.exists():
        # Try matching by SKILL.md name
        found = False
        for child in skills_dir.iterdir():
            if not child.is_dir() or not (child / "SKILL.md").exists():
                continue
            try:
                meta = parse_skill_metadata(child)
                if meta.name == skill_name:
                    skill_path = child
                    found = True
                    break
            except Exception:
                continue
        if not found:
            print(f"✗ Skill '{skill_name}' not found in {skills_dir}")
            return 1

    # Confirm removal
    force = getattr(args, "force", False)
    if not force:
        answer = input(f"Remove skill '{skill_path.name}' from {skills_dir}? [y/N] ")
        if answer.lower() not in ("y", "yes"):
            print("Cancelled.")
            return 0

    shutil.rmtree(skill_path)
    print(f"✓ Removed skill '{skill_path.name}'")
    return 0

