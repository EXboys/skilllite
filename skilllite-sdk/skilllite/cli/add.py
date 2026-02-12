"""
Add command for skilllite CLI.

Provides the ``skilllite add <source>`` command to download and install
skills from remote Git repositories or local paths.

Source formats supported:
  - GitHub shorthand: ``owner/repo`` or ``owner/repo/path``
  - Full GitHub URL: ``https://github.com/owner/repo``
  - GitHub tree URL: ``https://github.com/owner/repo/tree/branch/path``
  - Direct git URL: ``git@github.com:owner/repo.git``
  - Local path: ``./my-skills`` or ``/absolute/path``

Uses ``git clone --depth 1`` (shallow clone) for efficient downloading,
same approach as vercel-labs/skills.
"""

import argparse
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional

from ..core.metadata import parse_skill_metadata


# ---------------------------------------------------------------------------
# Source parsing
# ---------------------------------------------------------------------------

@dataclass
class ParsedSource:
    """Parsed result of a source string."""
    type: str  # "github", "gitlab", "git", "local"
    url: str
    ref: Optional[str] = None
    subpath: Optional[str] = None
    skill_filter: Optional[str] = None


def _is_local_path(source: str) -> bool:
    """Check if source is a local filesystem path."""
    return (
        os.path.isabs(source)
        or source.startswith("./")
        or source.startswith("../")
        or source in (".", "..")
    )


def parse_source(source: str) -> ParsedSource:
    """Parse a source string into a structured format.

    Supports GitHub shorthand, full URLs, git URLs, and local paths.
    """
    # Local path
    if _is_local_path(source):
        return ParsedSource(type="local", url=os.path.abspath(source))

    # GitHub tree URL with path: https://github.com/owner/repo/tree/branch/path
    m = re.search(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.+)", source)
    if m:
        owner, repo, ref, subpath = m.groups()
        return ParsedSource(
            type="github",
            url=f"https://github.com/{owner}/{repo}.git",
            ref=ref,
            subpath=subpath,
        )

    # GitHub tree URL with branch only: https://github.com/owner/repo/tree/branch
    m = re.search(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)$", source)
    if m:
        owner, repo, ref = m.groups()
        return ParsedSource(
            type="github",
            url=f"https://github.com/{owner}/{repo}.git",
            ref=ref,
        )

    # GitHub URL: https://github.com/owner/repo
    m = re.search(r"github\.com/([^/]+)/([^/]+?)(?:\.git)?/*$", source)
    if m:
        owner, repo = m.groups()
        return ParsedSource(type="github", url=f"https://github.com/{owner}/{repo}.git")

    # GitLab URL: https://gitlab.com/owner/repo
    m = re.search(r"gitlab\.com/(.+?)(?:\.git)?/?$", source)
    if m and "/" in m.group(1):
        repo_path = m.group(1)
        return ParsedSource(type="gitlab", url=f"https://gitlab.com/{repo_path}.git")

    # GitHub shorthand with @ filter: owner/repo@skill-name
    m = re.match(r"^([^/]+)/([^/@]+)@(.+)$", source)
    if m and ":" not in source:
        owner, repo, skill_filter = m.groups()
        return ParsedSource(
            type="github",
            url=f"https://github.com/{owner}/{repo}.git",
            skill_filter=skill_filter,
        )

    # GitHub shorthand: owner/repo or owner/repo/path/to/skill
    m = re.match(r"^([^/]+)/([^/]+)(?:/(.+))?$", source)
    if m and ":" not in source and not source.startswith("."):
        owner, repo, subpath = m.groups()
        return ParsedSource(
            type="github",
            url=f"https://github.com/{owner}/{repo}.git",
            subpath=subpath,
        )

    # Fallback: treat as direct git URL
    return ParsedSource(type="git", url=source)


# ---------------------------------------------------------------------------
# Git clone
# ---------------------------------------------------------------------------

CLONE_TIMEOUT_SECONDS = 60


def _clone_repo(url: str, ref: Optional[str] = None) -> str:
    """Shallow-clone a git repo and return the temp directory path.

    Raises ``RuntimeError`` on failure.
    """
    temp_dir = tempfile.mkdtemp(prefix="skilllite-")
    clone_cmd = ["git", "clone", "--depth", "1"]
    if ref:
        clone_cmd.extend(["--branch", ref])
    clone_cmd.extend([url, temp_dir])

    try:
        result = subprocess.run(
            clone_cmd,
            capture_output=True,
            text=True,
            timeout=CLONE_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        shutil.rmtree(temp_dir, ignore_errors=True)
        raise RuntimeError(
            f"Clone timed out after {CLONE_TIMEOUT_SECONDS}s. "
            "Check your network connection or repository access.\n"
            "  For SSH: ssh-add -l\n"
            "  For HTTPS: gh auth status"
        )

    if result.returncode != 0:
        shutil.rmtree(temp_dir, ignore_errors=True)
        stderr = result.stderr.strip()
        if "Authentication failed" in stderr or "Permission denied" in stderr:
            raise RuntimeError(
                f"Authentication failed for {url}.\n"
                "  For private repos, ensure you have access.\n"
                "  For SSH: ssh -T git@github.com\n"
                "  For HTTPS: gh auth login"
            )
        raise RuntimeError(f"Failed to clone {url}: {stderr}")

    return temp_dir


# ---------------------------------------------------------------------------
# Skill discovery
# ---------------------------------------------------------------------------

# Standard locations where skills may live inside a repository
_SKILL_SEARCH_DIRS = [
    "skills",
    ".skills",
    ".agents/skills",
    ".claude/skills",
    ".",  # root of repository
]


def _discover_skills(repo_dir: str, subpath: Optional[str] = None,
                     skill_filter: Optional[str] = None) -> List[Path]:
    """Discover skill directories (containing SKILL.md) inside *repo_dir*.

    If *subpath* is given, only search within that subdirectory.
    If *skill_filter* is given, only return skills whose directory name matches.
    """
    root = Path(repo_dir)
    candidates: List[Path] = []

    if subpath:
        # User pointed to a specific sub-path, e.g. "calculator"
        # 1) Check exact path: root/calculator/SKILL.md
        target = root / subpath
        if target.is_dir() and (target / "SKILL.md").exists():
            candidates.append(target)
            return candidates
        # 2) Check if subpath is a directory containing skills
        if target.is_dir():
            for child in sorted(target.iterdir()):
                if child.is_dir() and (child / "SKILL.md").exists():
                    candidates.append(child)
            if candidates:
                return candidates
        # 3) Search standard skill directories for a matching name
        #    e.g. subpath="calculator" -> check .skills/calculator, skills/calculator, etc.
        skill_name = subpath.split("/")[-1]  # last component as skill name
        for search_dir in _SKILL_SEARCH_DIRS:
            if search_dir == ".":
                continue
            candidate = root / search_dir / skill_name
            if candidate.is_dir() and (candidate / "SKILL.md").exists():
                candidates.append(candidate)
        # 4) Also try the full subpath under each standard directory
        if not candidates and "/" in subpath:
            for search_dir in _SKILL_SEARCH_DIRS:
                if search_dir == ".":
                    continue
                candidate = root / search_dir / subpath
                if candidate.is_dir() and (candidate / "SKILL.md").exists():
                    candidates.append(candidate)
        # 5) Fallback: recursive search for a directory matching the skill name
        if not candidates:
            for skill_md in root.rglob("SKILL.md"):
                if skill_md.parent.name == skill_name and ".git" not in skill_md.parts:
                    candidates.append(skill_md.parent)
        return candidates

    # Search standard locations
    seen: set = set()
    for search_dir in _SKILL_SEARCH_DIRS:
        search_path = root / search_dir
        if not search_path.is_dir():
            continue
        if search_dir == ".":
            # Check immediate subdirectories of root
            for child in sorted(search_path.iterdir()):
                if child.is_dir() and (child / "SKILL.md").exists():
                    real = child.resolve()
                    if real not in seen:
                        seen.add(real)
                        candidates.append(child)
        else:
            # Check the directory itself
            if (search_path / "SKILL.md").exists():
                real = search_path.resolve()
                if real not in seen:
                    seen.add(real)
                    candidates.append(search_path)
            # And its immediate children
            for child in sorted(search_path.iterdir()):
                if child.is_dir() and (child / "SKILL.md").exists():
                    real = child.resolve()
                    if real not in seen:
                        seen.add(real)
                        candidates.append(child)

    # Apply skill name filter if given
    if skill_filter:
        candidates = [
            c for c in candidates
            if c.name == skill_filter
        ]

    return candidates


# ---------------------------------------------------------------------------
# Skill copying
# ---------------------------------------------------------------------------

def _copy_skill(src: Path, dest: Path) -> None:
    """Copy a skill directory to *dest*, overwriting if it already exists."""
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(
        src, dest,
        ignore=shutil.ignore_patterns(".git", "__pycache__", "*.pyc", ".DS_Store"),
    )


# ---------------------------------------------------------------------------
# cmd_add - the CLI entry point
# ---------------------------------------------------------------------------

def cmd_add(args: argparse.Namespace) -> int:
    """Execute the ``skilllite add <source>`` command."""
    source_str: str = args.source
    skills_dir = Path(args.skills_dir or ".skills")
    force: bool = getattr(args, "force", False)
    list_only: bool = getattr(args, "list", False)

    # Make skills_dir absolute
    if not skills_dir.is_absolute():
        skills_dir = Path(os.getcwd()) / skills_dir

    parsed = parse_source(source_str)
    print(f"📦 Source: {source_str}")
    print(f"   Type: {parsed.type}")
    print(f"   URL: {parsed.url}")
    if parsed.ref:
        print(f"   Ref: {parsed.ref}")
    if parsed.subpath:
        print(f"   Subpath: {parsed.subpath}")
    if parsed.skill_filter:
        print(f"   Filter: {parsed.skill_filter}")
    print()

    # --- Clone or resolve local path ---
    temp_dir: Optional[str] = None

    try:
        if parsed.type == "local":
            repo_dir = parsed.url
            if not os.path.isdir(repo_dir):
                print(f"✗ Local path does not exist: {repo_dir}", flush=True)
                return 1
            print(f"📁 Using local path: {repo_dir}")
        else:
            print(f"⬇ Cloning {parsed.url} ...", flush=True)
            temp_dir = _clone_repo(parsed.url, parsed.ref)
            repo_dir = temp_dir
            print("✓ Clone complete")

        # --- Discover skills ---
        print()
        print("🔍 Discovering skills...")
        skills = _discover_skills(repo_dir, parsed.subpath, parsed.skill_filter)

        if not skills:
            print("   No skills found (no SKILL.md files detected)")
            return 1

        print(f"   Found {len(skills)} skill(s):")
        for s in skills:
            try:
                meta = parse_skill_metadata(s)
                name = meta.name or s.name
                desc = meta.description or ""
                print(f"   • {name}: {desc[:60]}")
            except Exception:
                print(f"   • {s.name}: (could not parse SKILL.md)")

        # --- List-only mode ---
        if list_only:
            return 0

        # --- Copy skills to .skills/ ---
        print()
        skills_dir.mkdir(parents=True, exist_ok=True)
        installed: List[str] = []

        for skill_path in skills:
            try:
                meta = parse_skill_metadata(skill_path)
                skill_name = meta.name or skill_path.name
            except Exception:
                skill_name = skill_path.name

            dest = skills_dir / skill_name
            if dest.exists() and not force:
                print(f"   ⏭ {skill_name}: already exists (use --force to overwrite)")
                continue

            _copy_skill(skill_path, dest)
            installed.append(skill_name)
            print(f"   ✓ {skill_name}: installed to {dest}")

        if not installed:
            print("   No new skills installed.")
            return 0

        # --- Install dependencies ---
        print()
        print("📦 Installing dependencies...")
        from .init_deps import scan_and_install_deps
        dep_results = scan_and_install_deps(skills_dir, force=force)
        for r in dep_results:
            # Only show results for newly-installed skills
            if r.get("name") not in installed:
                continue
            pkgs = r.get("packages", [])
            pkg_str = ", ".join(pkgs) if pkgs else "none"
            status = r.get("status", "unknown")
            lang = r.get("language", "")
            resolver = r.get("resolver", "")
            lang_tag = f" [{lang}]" if lang else ""
            resolver_tag = f" (via {resolver})" if resolver and resolver != "none" else ""
            if status.startswith("ok"):
                print(f"   ✓ {r['name']}{lang_tag}: {pkg_str}{resolver_tag}")
            else:
                print(f"   ✗ {r['name']}{lang_tag}: {status}")

        # --- Summary ---
        print()
        print("=" * 50)
        print(f"🎉 Successfully added {len(installed)} skill(s) from {source_str}")
        for name in installed:
            print(f"  • {name}")
        print("=" * 50)

        return 0

    except RuntimeError as e:
        print(f"\n✗ Error: {e}", flush=True)
        return 1
    except Exception as e:
        import traceback
        print(f"\n✗ Unexpected error: {e}", flush=True)
        traceback.print_exc()
        return 1
    finally:
        # Clean up temp directory
        if temp_dir and os.path.isdir(temp_dir):
            shutil.rmtree(temp_dir, ignore_errors=True)

