"""Package resolution: LLM inference, whitelist matching, registry validation."""

import json
import os
import re
from typing import List, Optional


def _get_known_packages():
    """Lazy load to avoid circular import at module load time."""
    from ...config.packages_whitelist import get_python_packages, get_python_aliases, get_node_packages
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


def _is_word_match(text: str, word: str) -> bool:
    """Check if *word* appears as a complete word in *text*."""
    pattern = r'(?<![a-zA-Z0-9])' + re.escape(word) + r'(?![a-zA-Z0-9])'
    return bool(re.search(pattern, text, re.IGNORECASE))


def validate_packages_whitelist(
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


def resolve_packages(
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
