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
"""

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

from ..sandbox.skillbox import (
    install as install_binary,
    is_installed,
    get_installed_version,
)
from ..core.metadata import (
    parse_skill_metadata,
    detect_language,
)


# ---------------------------------------------------------------------------
# Lock file constants
# ---------------------------------------------------------------------------

LOCK_FILE_NAME = ".skilllite.lock"


# ---------------------------------------------------------------------------
# Known packages list (single source: packages_whitelist.json)
# ---------------------------------------------------------------------------

def _get_known_packages():
    """Lazy load to avoid circular import at module load time."""
    from ..packages_whitelist import get_python_packages, get_python_aliases, get_node_packages
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
    """Validate packages against known whitelist. Returns (ok, unknown_packages).

    When packages come from .skilllite.lock, we check each against the whitelist.
    Non-whitelist packages are rejected unless allow_unknown is True.
    """
    if allow_unknown:
        return True, []

    whitelist = _known_python_set() if language == "python" else _known_node_set()

    unknown: List[str] = []
    for pkg in packages:
        base = pkg.split("[")[0].strip().lower()  # strip extras like "pillow[image]"
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


def _compute_packages_hash(packages: List[str]) -> str:
    """Compute SHA-256 hash from package list (mirrors Rust compute_packages_hash).

    The list is **sorted** before hashing so that different orderings of the
    same packages always produce the same hash.
    """
    h = hashlib.sha256()
    for pkg in sorted(packages):
        h.update(pkg.encode())
        h.update(b"\n")
    return h.hexdigest()


def _get_cache_key(language: str, content_hash: str) -> str:
    """Get cache key for a dependency configuration (mirrors Rust get_cache_key).

    Uses the same prefix mapping as Rust: python -> "py", node -> "node".
    """
    prefix_map = {"python": "py", "node": "node"}
    prefix = prefix_map.get(language, language)
    if not content_hash:
        return f"{prefix}-none"
    return f"{prefix}-{content_hash[:16]}"


def _get_cache_dir() -> Path:
    """Get the environment cache directory (mirrors Rust get_cache_dir)."""
    env_override = os.environ.get("AGENTSKILL_CACHE_DIR")
    if env_override:
        return Path(env_override) / "agentskill" / "envs"
    system = platform.system()
    if system == "Darwin":
        base = Path.home() / "Library" / "Caches"
    elif system == "Linux":
        xdg = os.environ.get("XDG_CACHE_HOME")
        base = Path(xdg) if xdg else Path.home() / ".cache"
    else:
        base = Path.home() / ".cache"
    return base / "agentskill" / "envs"


# ---------------------------------------------------------------------------
# .skilllite.lock I/O
# ---------------------------------------------------------------------------

def _read_lock_file(skill_dir: Path) -> Optional[Dict[str, Any]]:
    """Read and return the parsed ``.skilllite.lock`` for *skill_dir*.

    Returns ``None`` if the file does not exist or is invalid.
    """
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

    # Load .env if python-dotenv available
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
    """Use an LLM to extract package names from a compatibility string.

    Returns a list of package name strings, or ``None`` if the LLM call
    fails or no API key is configured.

    This function is only called when ``_is_llm_configured()`` returns True,
    so callers should gate on that first to avoid noisy warnings.
    """
    from openai import OpenAI

    # Load .env if python-dotenv available
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
        # Strip markdown code fences if present
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
    import requests as _requests  # avoid clash with the known-packages name

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
    """Resolve packages from *compatibility* using the best available strategy.

    Returns ``(packages: List[str], resolver: str)``.

    Strategy:
    1. If an LLM API is configured → call LLM + validate against PyPI/npm.
    2. Otherwise (or if LLM fails) → fall back to hardcoded whitelist matching.

    The LLM path is only attempted when ``_is_llm_configured()`` returns True,
    so users without an API key will never see LLM-related warnings.
    """
    if not compatibility:
        return [], "none"

    # --- Strategy 1: LLM extraction (only if configured) --------------------
    if _is_llm_configured():
        llm_packages = _extract_packages_with_llm(compatibility, language)
        if llm_packages is not None and len(llm_packages) > 0:
            # Validate each package against the registry
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

    # --- Strategy 2: Hardcoded whitelist fallback ----------------------------
    whitelist_packages = parse_compatibility_for_packages(compatibility)
    if whitelist_packages:
        return sorted(whitelist_packages), "whitelist"

    return [], "none"


# ---------------------------------------------------------------------------
# Hello-world skill template
# ---------------------------------------------------------------------------

_HELLO_SKILL_MD = (
    "---\n"
    "name: hello-world\n"
    "description: A simple hello-world skill for testing the SkillLite setup.\n"
    "license: MIT\n"
    "metadata:\n"
    "  author: skilllite-init\n"
    '  version: "1.0"\n'
    "---\n"
    "\n"
    "# Hello World Skill\n"
    "\n"
    "A minimal skill that echoes back a greeting.\n"
    "Use this to verify your SkillLite setup works.\n"
    "\n"
    "## Usage\n"
    "\n"
    "Provide a JSON input with a `name` field:\n"
    "\n"
    '```json\n{"name": "World"}\n```\n'
)

_HELLO_MAIN_PY = (
    "#!/usr/bin/env python3\n"
    '"""Hello-world skill entry point."""\n'
    "import json\n"
    "import sys\n"
    "\n"
    "\n"
    "def main():\n"
    "    data = json.loads(sys.stdin.read())\n"
    '    name = data.get("name", "World")\n'
    '    result = {"greeting": f"Hello, {name}!"}\n'
    "    print(json.dumps(result))\n"
    "\n"
    "\n"
    'if __name__ == "__main__":\n'
    "    main()\n"
)

# ---------------------------------------------------------------------------
# Data-analysis skill template (has pandas + numpy dependencies)
# ---------------------------------------------------------------------------

_DATA_ANALYSIS_SKILL_MD = (
    "---\n"
    "name: data-analysis\n"
    "description: Analyze CSV/JSON data with statistics, filtering, and aggregation. "
    "Powered by pandas and numpy.\n"
    "compatibility: Requires Python 3.x with pandas, numpy\n"
    "license: MIT\n"
    "metadata:\n"
    "  author: skilllite-init\n"
    '  version: "1.0"\n'
    "---\n"
    "\n"
    "# Data Analysis Skill\n"
    "\n"
    "Perform statistical analysis on tabular data using pandas and numpy.\n"
    "\n"
    "## Supported Operations\n"
    "\n"
    "- **describe**: Summary statistics (mean, std, min, max, etc.)\n"
    "- **filter**: Filter rows by column conditions\n"
    "- **aggregate**: Group-by aggregation (sum, mean, count, etc.)\n"
    "- **correlate**: Correlation matrix between numeric columns\n"
    "\n"
    "## Usage\n"
    "\n"
    '```json\n{"operation": "describe", "data": [[1,2],[3,4]], '
    '"columns": ["a","b"]}\n```\n'
)

_DATA_ANALYSIS_MAIN_PY = (
    "#!/usr/bin/env python3\n"
    '"""Data analysis skill entry point."""\n'
    "import json\n"
    "import sys\n"
    "\n"
    "import numpy as np\n"
    "import pandas as pd\n"
    "\n"
    "\n"
    "def main():\n"
    "    data = json.loads(sys.stdin.read())\n"
    '    operation = data.get("operation", "describe")\n'
    '    rows = data.get("data", [])\n'
    '    columns = data.get("columns")\n'
    "\n"
    "    df = pd.DataFrame(rows, columns=columns)\n"
    "\n"
    '    if operation == "describe":\n'
    "        result = json.loads(df.describe().to_json())\n"
    '    elif operation == "filter":\n'
    '        col = data.get("column", df.columns[0])\n'
    '        op = data.get("op", ">")\n'
    '        val = data.get("value", 0)\n'
    '        if op == ">":\n'
    "            filtered = df[df[col] > val]\n"
    '        elif op == "<":\n'
    "            filtered = df[df[col] < val]\n"
    '        elif op == "==":\n'
    "            filtered = df[df[col] == val]\n"
    "        else:\n"
    "            filtered = df\n"
    '        result = {"filtered": json.loads(filtered.to_json(orient="records")),\n'
    '                  "count": len(filtered)}\n'
    '    elif operation == "aggregate":\n'
    '        group_col = data.get("group_by", df.columns[0])\n'
    '        agg_col = data.get("agg_column", df.columns[-1])\n'
    '        agg_func = data.get("agg_func", "mean")\n'
    "        grouped = df.groupby(group_col)[agg_col].agg(agg_func)\n"
    "        result = json.loads(grouped.to_json())\n"
    '    elif operation == "correlate":\n'
    "        numeric = df.select_dtypes(include=[np.number])\n"
    "        result = json.loads(numeric.corr().to_json())\n"
    "    else:\n"
    '        result = {"error": f"Unknown operation: {operation}"}\n'
    "\n"
    "    print(json.dumps(result))\n"
    "\n"
    "\n"
    'if __name__ == "__main__":\n'
    "    main()\n"
)


# ---------------------------------------------------------------------------
# Environment setup (mirrors skillbox/src/env/builder.rs)
# ---------------------------------------------------------------------------

def _ensure_python_env(env_path: Path, packages: List[str]) -> None:
    """Create a Python venv and install *packages* into it."""
    marker = env_path / ".agentskill_complete"
    if env_path.exists() and marker.exists():
        # Env exists, only check if playwright needs chromium
        if "playwright" in packages:
            _ensure_playwright_chromium(env_path)
        return  # already done

    # Remove incomplete env
    if env_path.exists():
        shutil.rmtree(env_path)

    # Create venv
    result = subprocess.run(
        [sys.executable, "-m", "venv", str(env_path)],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Failed to create venv: {result.stderr}")

    # Install packages
    if packages:
        pip = env_path / ("Scripts" if os.name == "nt" else "bin") / "pip"
        result = subprocess.run(
            [str(pip), "install", "--quiet", "--disable-pip-version-check"] + packages,
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            raise RuntimeError(f"pip install failed: {result.stderr}")

    marker.write_text("")

    # Playwright needs browser install
    if "playwright" in packages:
        _ensure_playwright_chromium(env_path)


def _ensure_playwright_chromium(env_path: Path) -> None:
    """Run playwright install chromium in the given venv."""
    pw_marker = env_path / ".playwright_chromium_done"
    if pw_marker.exists():
        return
    python_bin = env_path / ("Scripts" if os.name == "nt" else "bin") / "python"
    result = subprocess.run(
        [str(python_bin), "-m", "playwright", "install", "chromium"],
        capture_output=True, text=True,
        timeout=300,
    )
    if result.returncode != 0:
        err = result.stderr or result.stdout or ""
        raise RuntimeError(
            f"playwright install chromium failed: {err}\n"
            "You can run manually later: playwright install chromium"
        )
    pw_marker.write_text("")


def _ensure_node_env(env_path: Path, packages: List[str]) -> None:
    """Create a Node.js environment directory and install *packages*."""
    marker = env_path / ".agentskill_complete"
    if env_path.exists() and marker.exists():
        return

    if env_path.exists():
        shutil.rmtree(env_path)

    env_path.mkdir(parents=True, exist_ok=True)

    if packages:
        result = subprocess.run(
            ["npm", "install", "--silent"] + packages,
            capture_output=True, text=True,
            cwd=str(env_path),
        )
        if result.returncode != 0:
            raise RuntimeError(f"npm install failed: {result.stderr}")

    marker.write_text("")


# ---------------------------------------------------------------------------
# Dependency security audit (pip-audit, npm audit)
# ---------------------------------------------------------------------------

def _audit_python_env(env_path: Path) -> tuple[bool, List[Dict[str, Any]], Optional[bool]]:
    """Run pip-audit on a Python venv. Returns (has_vulnerabilities, list_of_issues, pip_audit_available).

    Third value: True=pip-audit ran, False=not installed, None=error.
    """
    pip_bin = env_path / ("Scripts" if os.name == "nt" else "bin") / "pip"
    if not pip_bin.exists():
        return False, [], None

    # Get installed packages from venv
    freeze_result = subprocess.run(
        [str(pip_bin), "freeze"],
        capture_output=True, text=True, timeout=30,
    )
    if freeze_result.returncode != 0 or not freeze_result.stdout.strip():
        return False, [], None

    # Try to run pip-audit (may not be installed)
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

    # pip-audit not installed: ModuleNotFoundError produces exit code 1 and stderr
    stderr_lower = (result.stderr or "").lower()
    if "pip_audit" in stderr_lower or "no module named" in stderr_lower:
        return False, [], False

    # pip-audit exits 0=ok, 1=vulns found
    if result.returncode not in (0, 1):
        return False, [], None

    issues: List[Dict[str, Any]] = []
    try:
        data = json.loads(result.stdout)
        # pip-audit JSON: list of {name, version, vulns: [{id, fix_versions, ...}]}
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
        # Output might have a "Found N vulnerabilities..." line before JSON, try to extract
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
    """Run npm audit on a Node.js env. Returns (has_vulnerabilities, list_of_issues)."""
    package_json = env_path / "package.json"
    if not package_json.exists():
        return False, []

    result = subprocess.run(
        ["npm", "audit", "--json"],
        capture_output=True, text=True, timeout=60,
        cwd=str(env_path),
    )

    # npm audit exits 1 when vulns found, 0 when none
    if result.returncode not in (0, 1):
        return False, []

    issues: List[Dict[str, Any]] = []
    try:
        data = json.loads(result.stdout)
        # npm audit JSON: vulnerabilities dict, or metadata.vulnerabilities for counts
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
    """Run pip-audit / npm audit on each env. Returns (success, list of warning lines)."""
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
                for i in issues[:5]:  # Show first 5
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

def _scan_and_install_deps(
    skills_dir: Path,
    force: bool = False,
    allow_unknown_packages: bool = False,
) -> List[Dict]:
    """Scan all skills under *skills_dir*, resolve dependencies, and install
    them into isolated environments.

    Resolution order per skill:
    1. Read ``.skilllite.lock`` — if present and not stale, use cached packages.
    2. LLM inference + PyPI/npm registry validation (requires API key).
    3. Hardcoded whitelist matching (offline fallback).

    After resolution the result is persisted in ``.skilllite.lock`` so that
    ``skillbox run`` can read it without calling an LLM.

    Returns a list of dicts describing each skill and its dependency status.
    """
    cache_dir = _get_cache_dir()
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

        # ---- Resolve packages (lock → LLM → whitelist) --------------------
        resolver = "none"
        packages: List[str] = []

        lock_data = _read_lock_file(skill_path)
        if lock_data and not force and not _lock_is_stale(lock_data, metadata.compatibility):
            # Use cached resolution
            packages = lock_data.get("resolved_packages", [])
            resolver = "lock"
            # Validate lock packages against whitelist (supply chain security)
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
            # Resolve from scratch
            packages, resolver = _resolve_packages(metadata.compatibility, language)
            # Persist to lock file
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

        content_hash = _compute_packages_hash(packages)
        cache_key = _get_cache_key(language, content_hash)
        env_path = cache_dir / cache_key

        try:
            if language == "python":
                _ensure_python_env(env_path, packages)
            elif language == "node":
                _ensure_node_env(env_path, packages)
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


# ---------------------------------------------------------------------------
# cmd_init - the CLI entry point
# ---------------------------------------------------------------------------

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
        if not getattr(args, "skip_binary", False):
            if is_installed():
                version = get_installed_version()
                print(f"\u2713 skillbox binary already installed (v{version})")
            else:
                print("\u2b07 Installing skillbox binary...")
                install_binary(show_progress=True)
        else:
            print("\u23ed Skipping binary installation (--skip-binary)")

        # -- Step 2: .skills directory & hello-world -----------------------
        created_files: List[str] = []

        if not skills_dir.exists():
            skills_dir.mkdir(parents=True, exist_ok=True)
            print(f"\u2713 Created skills directory: {skills_dir_rel}")
        else:
            print(f"\u2713 Skills directory already exists: {skills_dir_rel}")

        hello_dir = skills_dir / "hello-world"
        if not hello_dir.exists() or getattr(args, "force", False):
            hello_dir.mkdir(parents=True, exist_ok=True)
            (hello_dir / "SKILL.md").write_text(_HELLO_SKILL_MD, encoding="utf-8")
            scripts_dir = hello_dir / "scripts"
            scripts_dir.mkdir(parents=True, exist_ok=True)
            (scripts_dir / "main.py").write_text(_HELLO_MAIN_PY, encoding="utf-8")
            created_files.extend([
                f"{skills_dir_rel}/hello-world/SKILL.md",
                f"{skills_dir_rel}/hello-world/scripts/main.py",
            ])
            print("\u2713 Created hello-world example skill")
        else:
            print("\u2713 hello-world skill already exists (use --force to overwrite)")

        analysis_dir = skills_dir / "data-analysis"
        if not analysis_dir.exists() or getattr(args, "force", False):
            analysis_dir.mkdir(parents=True, exist_ok=True)
            (analysis_dir / "SKILL.md").write_text(
                _DATA_ANALYSIS_SKILL_MD, encoding="utf-8")
            da_scripts = analysis_dir / "scripts"
            da_scripts.mkdir(parents=True, exist_ok=True)
            (da_scripts / "main.py").write_text(
                _DATA_ANALYSIS_MAIN_PY, encoding="utf-8")
            created_files.extend([
                f"{skills_dir_rel}/data-analysis/SKILL.md",
                f"{skills_dir_rel}/data-analysis/scripts/main.py",
            ])
            print("\u2713 Created data-analysis example skill (pandas, numpy)")
        else:
            print("\u2713 data-analysis skill already exists (use --force to overwrite)")

        # -- Step 3: Scan & install dependencies ---------------------------
        force = getattr(args, "force", False)
        allow_unknown = getattr(args, "allow_unknown_packages", False) or (
            os.environ.get("SKILLLITE_ALLOW_UNKNOWN_PACKAGES", "").lower()
            in ("1", "true", "yes")
        )
        dep_results: List[Dict[str, Any]] = []
        if not getattr(args, "skip_deps", False):
            print()
            print("\U0001f4e6 Scanning skills and installing dependencies...")
            dep_results = _scan_and_install_deps(
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
            audit_ok, audit_lines = _run_dependency_audits(
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