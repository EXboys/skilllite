"""
Standalone binary download and install logic.

Extracted from binary.py for Phase 4.10 slim. Uses only stdlib.
"""

import os
import shutil
import stat
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
from pathlib import Path
from typing import Optional

from ...logger import get_logger

logger = get_logger("skilllite.sandbox.core._install", verbose=True)

BINARY_VERSION = "0.2.0"
GITHUB_OWNER = "EXboys"
GITHUB_REPO = "skilllite"
BINARY_NAME = "skilllite"

PLATFORM_MAP = {
    ("Darwin", "arm64"): "darwin-arm64",
    ("Darwin", "x86_64"): "darwin-x64",
    ("Linux", "x86_64"): "linux-x64",
    ("Linux", "aarch64"): "linux-arm64",
    ("Linux", "arm64"): "linux-arm64",
}


def get_platform() -> str:
    import platform
    key = (platform.system(), platform.machine())
    if key not in PLATFORM_MAP:
        raise RuntimeError(
            f"Unsupported platform: {key[0]} {key[1]}. "
            f"Supported: macOS (x64, arm64), Linux (x64, arm64)"
        )
    return PLATFORM_MAP[key]


def get_install_dir() -> Path:
    return Path.home() / ".skilllite" / "bin"


def get_binary_path() -> Path:
    return get_install_dir() / BINARY_NAME


def get_version_file() -> Path:
    return Path.home() / ".skilllite" / ".version"


def get_download_url(version: Optional[str] = None) -> str:
    v = version or BINARY_VERSION
    plat = get_platform()
    base = f"https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases/download"
    return f"{base}/v{v}/skilllite-{plat}.tar.gz"


def _download(url: str, dest: Path, show_progress: bool = True) -> None:
    def _reporthook(block_num: int, block_size: int, total: int) -> None:
        if not show_progress or total <= 0:
            return
        done = block_num * block_size
        pct = min(100, done * 100 // total)
        bar = "█" * (40 * pct // 100) + "░" * (40 - 40 * pct // 100)
        sys.stdout.write(f"\r  Downloading: [{bar}] {pct}%")
        sys.stdout.flush()
        if done >= total:
            sys.stdout.write("\n")
            sys.stdout.flush()

    try:
        urllib.request.urlretrieve(url, dest, reporthook=_reporthook if show_progress else None)
    except urllib.error.HTTPError as e:
        if e.code == 404:
            raise RuntimeError(
                f"Binary not found at {url}. Check if v{BINARY_VERSION} has been released."
            ) from e
        raise


def _extract(archive: Path, dest: Path) -> Path:
    with tarfile.open(archive, "r:gz") as tar:
        tar.extractall(dest)
    binary = dest / BINARY_NAME
    if binary.exists():
        return binary
    for item in dest.iterdir():
        if item.is_dir() and (item / BINARY_NAME).exists():
            return item / BINARY_NAME
    raise RuntimeError(f"Binary not found in archive: {archive}")


def do_install(
    version: Optional[str] = None,
    show_progress: bool = True,
) -> Path:
    """
    Download and install the skilllite binary.
    Caller (binary.py) should check is_installed/needs_update before calling.
    """
    v = version or BINARY_VERSION
    plat = get_platform()
    logger.info(f"Installing skilllite v{v} for {plat}...")
    install_dir = get_install_dir()
    install_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        archive = tmp_path / f"skilllite-{plat}.tar.gz"
        url = get_download_url(v)
        logger.info(f"  Downloading from: {url}")
        _download(url, archive, show_progress)
        logger.info("  Extracting...")
        extracted = _extract(archive, tmp_path)
        dest = get_binary_path()
        if dest.exists():
            dest.unlink()
        shutil.move(str(extracted), str(dest))
        dest.chmod(dest.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

    version_file = get_version_file()
    version_file.parent.mkdir(parents=True, exist_ok=True)
    version_file.write_text(v)
    logger.info(f"✓ Successfully installed skilllite v{v}")
    logger.info(f"  Location: {dest}")
    return dest
