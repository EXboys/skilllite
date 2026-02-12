"""
Environment builder for skill dependency isolation.

Creates Python venvs and Node.js envs with package installation.
Mirrors skillbox/src/env/builder.rs.
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import List


def ensure_python_env(env_path: Path, packages: List[str]) -> None:
    """Create a Python venv and install *packages* into it."""
    marker = env_path / ".agentskill_complete"
    if env_path.exists() and marker.exists():
        # Env exists, only check if playwright needs chromium
        if "playwright" in packages:
            ensure_playwright_chromium(env_path)
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
        ensure_playwright_chromium(env_path)


def ensure_playwright_chromium(env_path: Path) -> None:
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


def ensure_node_env(env_path: Path, packages: List[str]) -> None:
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
