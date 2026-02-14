"""Dependency security audit: pip-audit, npm audit."""

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional


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


def run_dependency_audits(
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
                lines.append("   ℹ pip-audit not installed; skip Python audit. Install with: pip install pip-audit")
                continue
            if pip_audit_available is None:
                pip_audit_available = True
            if vuln:
                has_vulns = True
                lines.append(f"   ⚠️ {name} [Python]: {len(issues)} known vulnerability(ies)")
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
                lines.append(f"   ⚠️ {name} [Node]: {len(issues)} known vulnerability(ies)")
                for i in issues[:5]:
                    lines.append(f"      - {i.get('package', '?')} {i.get('version', '?')}: {i.get('id', '?')} ({i.get('severity', '?')})")
                if len(issues) > 5:
                    lines.append(f"      ... and {len(issues) - 5} more")

    success = not (strict and has_vulns)
    return success, lines
