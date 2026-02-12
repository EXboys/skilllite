"""
Dependency resolution and installation for skilllite init.

Handles lock file I/O, package resolution (LLM/whitelist), venv creation,
and security audit (pip-audit, npm audit).
"""

import hashlib
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

from ..core.metadata import parse_skill_metadata, detect_language
from ..isolation import (
    get_cache_dir,
    get_cache_key,
    compute_packages_hash,
    ensure_python_env,
    ensure_node_env,
)


LOCK_FILE_NAME = ".skilllite.lock"


# ---------------------------------------------------------------------------
# Known packages list (single source: packages_whitelist.json)
# ---------------------------------------------------------------------------

def _get_known_packages():
    """Lazy load to avoid circular import at module load time."""
    from ..config.packages_whitelist import get_python_packages, get_python_aliases, get_node_packages
    python = get_python_packages()
    aliases = get_python_aliases()
    node = get_node_packages()
    return python, aliases, node


def _known_python_set():
    python, aliases, _ = _get_known_packages()
    return {p.lower() for p in python} | {a.lower() for a in aliases}


def _known_node_set():
    _, _, node = _get_known_packages()
    return {p.lower() for p in node}


# ---------------------------------------------------------------------------
# Package parsing (mirrors Rust deps.rs logic)
# ---------------------------------------------------------------------------

def _is_word_match(text: str, word: str) -> bool:
    """Check if *word* appears as a complete word in *text*."""
    pattern = r'(?<![a-zA-Z0-9])' + re.escape(word) + r'(?![a-zA-Z0-9])'
    return bool(re.search(pattern, text, re.IGNORECASE))


def _validate_packages_whitelist(
    packages: List[str],
    language: str,
    allow_unknown: bool,
) -> tuple[bool, List[str]]:
    """Validate packages against known whitelist. Returns (ok, unknown_packages)."""
    if allow_unknown:
        return True, []

    whitelist = _known_python_set() if language == "python" else _known_node_set()
    unknown: List[str] = []
    for pkg in packages:
        base = pkg.split("[")[0].strip().lower()
        if base not in whitelist:
            unknown.append(pkg)
    return len(unknown) == 0, unknown


def parse_compatibility_for_packages(compatibility: Optional[str]) -> List[str]:
    """Parse compatibility string to extract package names.

    Mirrors ``skillbox/src/skill/deps.rs::parse_compatibility_for_packages``.
    Uses packages_whitelist.json as single source of truth.
    """
    if not compatibility:
        return []
    python_pkgs, _, node_pkgs = _get_known_packages()
    packages: List[str] = []
    for pkg in python_pkgs:
        if _is_word_match(compatibility, pkg):
            packages.append(pkg)
    for pkg in node_pkgs:
        if _is_word_match(compatibility, pkg):
            packages.append(pkg)
    return packages


# ---------------------------------------------------------------------------
# .skilllite.lock I/O
# ---------------------------------------------------------------------------

def _read_lock_file(skill_dir: Path) -> Optional[Dict[str, Any]]:
    """Read and return the parsed ``.skilllite.lock`` for *skill_dir*."""
    lock_path = skill_dir / LOCK_FILE_NAME
    if not lock_path.exists():
        return None
    try:
        return json.loads(lock_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def _write_lock_file(
    skill_dir: Path,
    compatibility: Optional[str],
    language: str,
    packages: List[str],
    resolver: str = "whitelist",
) -> None:
    """Write a ``.skilllite.lock`` file into *skill_dir*."""
    compat_hash = hashlib.sha256(
        (compatibility or "").encode()
    ).hexdigest()
    lock_data = {
        "compatibility_hash": compat_hash,
        "language": language,
        "resolved_packages": sorted(packages),
        "resolved_at": datetime.now(timezone.utc).isoformat(),
        "resolver": resolver,
    }
    lock_path = skill_dir / LOCK_FILE_NAME
    lock_path.write_text(
        json.dumps(lock_data, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def _lock_is_stale(
    lock_data: Dict[str, Any],
    compatibility: Optional[str],
) -> bool:
    """Return ``True`` if *lock_data* does not match the current compatibility."""
    current_hash = hashlib.sha256(
        (compatibility or "").encode()
    ).hexdigest()
    return lock_data.get("compatibility_hash") != current_hash


# ---------------------------------------------------------------------------
# LLM-based package extraction + registry validation
# ---------------------------------------------------------------------------

def _is_llm_configured() -> bool:
    """Return ``True`` if an LLM API is configured (openai lib + env vars)."""
    try:
        import openai as _openai  # noqa: F401
    except ImportError:
        return False
    try:
        from dotenv import load_dotenv
        load_dotenv()
    except ImportError:
        pass
    return bool(os.environ.get("BASE_URL") and os.environ.get("API_KEY"))


def _extract_packages_with_llm(
    compatibility: str,
    language: str,
) -> Optional[List[str]]:
    """Use an LLM to extract package names from a compatibility string."""
    from openai import OpenAI

    try:
        from dotenv import load_dotenv
        load_dotenv()
    except ImportError:
        pass

    base_url = os.environ.get("BASE_URL")
    api_key = os.environ.get("API_KEY")
    model = os.environ.get("MODEL", "deepseek-chat")
    lang_label = "Python (PyPI)" if language == "python" else "Node.js (npm)"

    prompt = (
        f"From the following compatibility/requirements string, extract the "
        f"{lang_label} package names that need to be installed via pip/npm.\n\n"
        f'"{compatibility}"\n\n'
        f"Rules:\n"
        f"- Only return real, installable package names (e.g. 'pandas', 'numpy', 'tqdm').\n"
        f"- Do NOT include language runtimes (python, node, bash) or generic words "
        f"(library, network, access, internet).\n"
        f"- Do NOT include version specifiers.\n"
        f"- Return ONLY a JSON array of strings. No explanation.\n"
        f'Example: ["pandas", "numpy", "tqdm"]'
    )

    try:
        client = OpenAI(base_url=base_url, api_key=api_key)
        response = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": prompt}],
            temperature=0,
        )
        content = response.choices[0].message.content.strip()
        if content.startswith("```"):
            lines = content.splitlines()
            lines = [l for l in lines if not l.startswith("```")]
            content = "\n".join(lines).strip()
        packages = json.loads(content)
        if isinstance(packages, list) and all(isinstance(p, str) for p in packages):
            return [p.strip().lower() for p in packages if p.strip()]
    except Exception as exc:
        print(f"   ⚠ LLM extraction failed: {exc}")

    return None


def _validate_package(package: str, language: str) -> bool:
    """Check whether *package* exists on PyPI or npm registry."""
    import requests as _requests

    try:
        if language == "python":
            url = f"https://pypi.org/pypi/{package}/json"
        elif language == "node":
            url = f"https://registry.npmjs.org/{package}"
        else:
            return False
        resp = _requests.get(url, timeout=8)
        return resp.status_code == 200
    except Exception:
        return False


def _resolve_packages(
    compatibility: Optional[str],
    language: str,
) -> tuple:
    """Resolve packages from *compatibility* using the best available strategy."""
    if not compatibility:
        return [], "none"

    if _is_llm_configured():
        llm_packages = _extract_packages_with_llm(compatibility, language)
        if llm_packages is not None and len(llm_packages) > 0:
            validated: List[str] = []
            invalid: List[str] = []
            for pkg in llm_packages:
                if _validate_package(pkg, language):
                    validated.append(pkg)
                else:
                    invalid.append(pkg)
            if invalid:
                print(f"   ⚠ Packages not found in registry (skipped): {', '.join(invalid)}")
            if validated:
                return sorted(validated), "llm"

    whitelist_packages = parse_compatibility_for_packages(compatibility)
    if whitelist_packages:
        return sorted(whitelist_packages), "whitelist"

    return [], "none"


# ---------------------------------------------------------------------------
# Dependency security audit (pip-audit, npm audit)
# ---------------------------------------------------------------------------

def _audit_python_env(env_path: Path) -> tuple[bool, List[Dict[str, Any]], Optional[bool]]:
    """Run pip-audit on a Python venv."""
    pip_bin = env_path / ("Scripts" if os.name == "nt" else "bin") / "pip"
    if not pip_bin.exists():
        return False, [], None

    freeze_result = subprocess.run(
        [str(pip_bin), "freeze"],
        capture_output=True, text=True, timeout=30,
    )
    if freeze_result.returncode != 0 or not freeze_result.stdout.strip():
        return False, [], None

    import tempfile
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".txt", delete=False
    ) as f:
        f.write(freeze_result.stdout)
        req_path = f.name

    try:
        result = subprocess.run(
            [sys.executable, "-m", "pip_audit", "-r", req_path, "-f", "json", "--progress-spinner", "off"],
            capture_output=True, text=True, timeout=60,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False, [], None
    finally:
        try:
            os.unlink(req_path)
        except OSError:
            pass

    stderr_lower = (result.stderr or "").lower()
    if "pip_audit" in stderr_lower or "no module named" in stderr_lower:
        return False, [], False

    if result.returncode not in (0, 1):
        return False, [], None

    issues: List[Dict[str, Any]] = []
    try:
        data = json.loads(result.stdout)
        if isinstance(data, list):
            for pkg in data:
                vulns = pkg.get("vulns", [])
                if vulns:
                    for v in vulns:
                        issues.append({
                            "package": pkg.get("name", "?"),
                            "version": pkg.get("version", "?"),
                            "id": v.get("id", "?"),
                            "fix_versions": v.get("fix_versions", []),
                        })
    except json.JSONDecodeError:
        for line in result.stdout.splitlines():
            if line.strip().startswith("["):
                try:
                    data = json.loads(line)
                    if isinstance(data, list):
                        for pkg in data:
                            vulns = pkg.get("vulns", [])
                            if vulns:
                                for v in vulns:
                                    issues.append({
                                        "package": pkg.get("name", "?"),
                                        "version": pkg.get("version", "?"),
                                        "id": v.get("id", "?"),
                                        "fix_versions": v.get("fix_versions", []),
                                    })
                except json.JSONDecodeError:
                    pass
                break

    return len(issues) > 0, issues, True


def _audit_node_env(env_path: Path) -> tuple[bool, List[Dict[str, Any]]]:
    """Run npm audit on a Node.js env."""
    package_json = env_path / "package.json"
    if not package_json.exists():
        return False, []

    result = subprocess.run(
        ["npm", "audit", "--json"],
        capture_output=True, text=True, timeout=60,
        cwd=str(env_path),
    )

    if result.returncode not in (0, 1):
        return False, []

    issues: List[Dict[str, Any]] = []
    try:
        data = json.loads(result.stdout)
        meta = data.get("metadata", {}).get("vulnerabilities", {})
        total = sum(
            int(meta.get(k, 0) or 0)
            for k in ("info", "low", "moderate", "high", "critical")
        )
        if total == 0:
            return False, []

        vulns_obj = data.get("vulnerabilities") or {}
        if isinstance(vulns_obj, dict):
            for name, info in vulns_obj.items():
                if isinstance(info, dict):
                    via = info.get("via")
                    severity = "?"
                    vuln_id = "?"
                    if isinstance(via, dict):
                        severity = via.get("severity", "?")
                        vuln_id = via.get("url", via.get("source", "?"))
                    elif isinstance(via, list) and via:
                        v0 = via[0]
                        if isinstance(v0, dict):
                            severity = v0.get("severity", "?")
                            vuln_id = v0.get("url", v0.get("source", "?"))
                    issues.append({
                        "package": name,
                        "version": info.get("version", "?"),
                        "id": vuln_id,
                        "severity": severity,
                    })
    except (json.JSONDecodeError, TypeError):
        pass

    return len(issues) > 0, issues


def _run_dependency_audits(
    dep_results: List[Dict],
    strict: bool = False,
    skip_audit: bool = False,
) -> tuple[bool, List[str]]:
    """Run pip-audit / npm audit on each env."""
    if skip_audit:
        return True, []

    lines: List[str] = []
    has_vulns = False
    pip_audit_available: Optional[bool] = None

    for r in dep_results:
        if r.get("status") != "ok" or "env_path" not in r:
            continue
        env_path = Path(r["env_path"])
        lang = r.get("language", "")
        name = r.get("name", "?")

        if lang == "python":
            if pip_audit_available is False:
                continue
            vuln, issues, avail = _audit_python_env(env_path)
            if avail is False:
                pip_audit_available = False
                lines.append("   \u2139 pip-audit not installed; skip Python audit. Install with: pip install pip-audit")
                continue
            if pip_audit_available is None:
                pip_audit_available = True
            if vuln:
                has_vulns = True
                lines.append(f"   \u26a0\ufe0f {name} [Python]: {len(issues)} known vulnerability(ies)")
                for i in issues[:5]:
                    fix = f" (fix: {', '.join(i.get('fix_versions', []))})" if i.get("fix_versions") else ""
                    lines.append(f"      - {i.get('package', '?')} {i.get('version', '?')}: {i.get('id', '?')}{fix}")
                if len(issues) > 5:
                    lines.append(f"      ... and {len(issues) - 5} more")

        elif lang == "node":
            try:
                vuln, issues = _audit_node_env(env_path)
            except Exception:
                continue
            if vuln:
                has_vulns = True
                lines.append(f"   \u26a0\ufe0f {name} [Node]: {len(issues)} known vulnerability(ies)")
                for i in issues[:5]:
                    lines.append(f"      - {i.get('package', '?')} {i.get('version', '?')}: {i.get('id', '?')} ({i.get('severity', '?')})")
                if len(issues) > 5:
                    lines.append(f"      ... and {len(issues) - 5} more")

    success = not (strict and has_vulns)
    return success, lines


# ---------------------------------------------------------------------------
# Skill scanning & dependency installation
# ---------------------------------------------------------------------------

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

        lock_data = _read_lock_file(skill_path)
        if lock_data and not force and not _lock_is_stale(lock_data, metadata.compatibility):
            packages = lock_data.get("resolved_packages", [])
            resolver = "lock"
            if packages and language in ("python", "node"):
                ok, unknown = _validate_packages_whitelist(
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
            packages, resolver = _resolve_packages(metadata.compatibility, language)
            _write_lock_file(skill_path, metadata.compatibility, language, packages, resolver)

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


def run_dependency_audits(
    dep_results: List[Dict],
    strict: bool = False,
    skip_audit: bool = False,
) -> tuple[bool, List[str]]:
    """Run pip-audit / npm audit on each env. Returns (success, list of warning lines)."""
    return _run_dependency_audits(dep_results, strict=strict, skip_audit=skip_audit)
