"""
Dependency resolution and installation for skilllite init.

Handles lock file I/O, package resolution (LLM/whitelist), venv creation,
and security audit (pip-audit, npm audit).
"""

from pathlib import Path
from typing import Any, Dict, List

from ...core.metadata import parse_skill_metadata, detect_language
from ...isolation import (
    get_cache_dir,
    get_cache_key,
    compute_packages_hash,
    ensure_python_env,
    ensure_node_env,
)

from .lock import read_lock_file, write_lock_file, lock_is_stale, LOCK_FILE_NAME
from .resolve import parse_compatibility_for_packages, validate_packages_whitelist, resolve_packages
from .audit import run_dependency_audits


__all__ = [
    "scan_and_install_deps",
    "run_dependency_audits",
    "parse_compatibility_for_packages",
    "LOCK_FILE_NAME",
]


def scan_and_install_deps(
    skills_dir: Path,
    force: bool = False,
    allow_unknown_packages: bool = False,
) -> List[Dict]:
    """Scan all skills under *skills_dir*, resolve dependencies, and install them.

    Resolution order per skill:
    1. Read ``.skilllite.lock`` — if present and not stale, use cached packages.
    2. LLM inference + PyPI/npm registry validation (requires API key).
    3. Hardcoded whitelist matching (offline fallback).

    Returns a list of dicts describing each skill and its dependency status.
    """
    cache_dir = get_cache_dir()
    cache_dir.mkdir(parents=True, exist_ok=True)

    results: List[Dict] = []

    if not skills_dir.exists():
        return results

    for skill_path in sorted(skills_dir.iterdir()):
        if not skill_path.is_dir():
            continue
        skill_md = skill_path / "SKILL.md"
        if not skill_md.exists():
            continue

        try:
            metadata = parse_skill_metadata(skill_path)
        except Exception as e:
            results.append({
                "name": skill_path.name,
                "status": "error",
                "error": f"Failed to parse SKILL.md: {e}",
            })
            continue

        language = detect_language(skill_path, metadata)
        resolver = "none"
        packages: List[str] = []

        lock_data = read_lock_file(skill_path)
        if lock_data and not force and not lock_is_stale(lock_data, metadata.compatibility):
            packages = lock_data.get("resolved_packages", [])
            resolver = "lock"
            if packages and language in ("python", "node"):
                ok, unknown = validate_packages_whitelist(
                    packages, language, allow_unknown_packages
                )
                if not ok:
                    results.append({
                        "name": metadata.name or skill_path.name,
                        "language": language,
                        "packages": packages,
                        "resolver": resolver,
                        "status": "error",
                        "error": (
                            f"Packages not in whitelist: {', '.join(unknown)}. "
                            "Run with --allow-unknown-packages to override, or update SKILL.md compatibility."
                        ),
                    })
                    continue
        else:
            packages, resolver = resolve_packages(metadata.compatibility, language)
            write_lock_file(skill_path, metadata.compatibility, language, packages, resolver)

        if not packages:
            results.append({
                "name": metadata.name or skill_path.name,
                "language": language,
                "packages": [],
                "resolver": resolver,
                "status": "ok (no deps)",
            })
            continue

        content_hash = compute_packages_hash(packages)
        cache_key = get_cache_key(language, content_hash)
        env_path = cache_dir / cache_key

        try:
            if language == "python":
                ensure_python_env(env_path, packages)
            elif language == "node":
                ensure_node_env(env_path, packages)
            else:
                results.append({
                    "name": metadata.name or skill_path.name,
                    "language": language,
                    "packages": packages,
                    "resolver": resolver,
                    "status": f"skipped (unsupported language: {language})",
                })
                continue

            results.append({
                "name": metadata.name or skill_path.name,
                "language": language,
                "packages": packages,
                "env_path": str(env_path),
                "resolver": resolver,
                "status": "ok",
            })
        except Exception as e:
            results.append({
                "name": metadata.name or skill_path.name,
                "language": language,
                "packages": packages,
                "resolver": resolver,
                "status": f"error: {e}",
            })

    return results
