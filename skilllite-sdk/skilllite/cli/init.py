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
# Known packages list (ported from skillbox/src/skill/deps.rs)
# ---------------------------------------------------------------------------

KNOWN_PYTHON_PACKAGES = [
    "requests", "pandas", "numpy", "scipy", "matplotlib", "seaborn",
    "sklearn", "scikit-learn", "tensorflow", "torch", "pytorch",
    "flask", "django", "fastapi", "aiohttp", "httpx",
    "beautifulsoup", "bs4", "lxml", "selenium",
    "pillow", "opencv", "cv2", "pyyaml", "yaml",
    "sqlalchemy", "psycopg2", "pymysql", "redis", "pymongo",
    "boto3", "google-cloud", "azure",
    "pytest", "unittest", "mock",
    "click", "argparse", "typer",
    "pydantic", "dataclasses", "attrs",
    "jinja2", "mako",
    "celery", "rq",
    "cryptography", "jwt", "passlib",
    "playwright",
]

KNOWN_NODE_PACKAGES = [
    "axios", "node-fetch", "got",
    "express", "koa", "fastify", "hapi",
    "lodash", "underscore", "ramda",
    "moment", "dayjs", "date-fns",
    "cheerio", "puppeteer", "playwright",
    "mongoose", "sequelize", "knex", "prisma",
    "ioredis",
    "aws-sdk", "googleapis",
    "jest", "mocha", "chai",
    "commander", "yargs", "inquirer",
    "chalk", "ora", "boxen",
    "dotenv",
    "jsonwebtoken", "bcrypt", "crypto-js",
    "socket.io", "ws",
    "sharp", "jimp",
]


# ---------------------------------------------------------------------------
# Package parsing (mirrors Rust deps.rs logic)
# ---------------------------------------------------------------------------

def _is_word_match(text: str, word: str) -> bool:
    """Check if *word* appears as a complete word in *text*."""
    pattern = r'(?<![a-zA-Z0-9])' + re.escape(word) + r'(?![a-zA-Z0-9])'
    return bool(re.search(pattern, text, re.IGNORECASE))


def parse_compatibility_for_packages(compatibility: Optional[str]) -> List[str]:
    """Parse compatibility string to extract package names.

    Mirrors ``skillbox/src/skill/deps.rs::parse_compatibility_for_packages``.
    """
    if not compatibility:
        return []
    packages: List[str] = []
    for pkg in KNOWN_PYTHON_PACKAGES:
        if _is_word_match(compatibility, pkg):
            packages.append(pkg)
    for pkg in KNOWN_NODE_PACKAGES:
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
# Skill scanning & dependency installation
# ---------------------------------------------------------------------------

def _scan_and_install_deps(skills_dir: Path, force: bool = False) -> List[Dict]:
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
        if not getattr(args, "skip_deps", False):
            print()
            print("\U0001f4e6 Scanning skills and installing dependencies...")
            dep_results = _scan_and_install_deps(skills_dir, force=force)

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
        else:
            print()
            print("\u23ed Skipping dependency installation (--skip-deps)")

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