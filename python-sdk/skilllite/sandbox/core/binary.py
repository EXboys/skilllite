"""
Binary management for the skilllite sandbox executor.

Handles finding, ensuring, and managing the Rust-based skilllite binary.
Download/install logic is in _install.py (Phase 4.10 slim).
"""

import os
import shutil
import subprocess
import warnings
from pathlib import Path
from typing import Optional

from ...logger import get_logger

logger = get_logger("skilllite.sandbox.core.binary", verbose=True)

# Re-export from _install for backward compatibility
from ._install import (
    BINARY_NAME,
    BINARY_VERSION,
    get_download_url,
    get_install_dir,
    get_platform,
)

_binary_path_cache: Optional[str] = None
_binary_path_cache_set: bool = False


def get_binary_path() -> Path:
    """Path to ~/.skilllite/bin/skilllite"""
    return get_install_dir() / BINARY_NAME


def get_version_file() -> Path:
    """Path to ~/.skilllite/.version"""
    return Path.home() / ".skilllite" / ".version"


def is_installed() -> bool:
    """True if binary exists and is executable."""
    return get_binary_path().exists() and os.access(get_binary_path(), os.X_OK)


def get_installed_version() -> Optional[str]:
    """Version string from .version file, or None."""
    vf = get_version_file()
    return vf.read_text().strip() if vf.exists() else None


def needs_update(target_version: Optional[str] = None) -> bool:
    """True if update needed."""
    if not is_installed():
        return True
    tv = target_version or BINARY_VERSION
    iv = get_installed_version()
    return iv is None or iv != tv


def invalidate_binary_cache() -> None:
    """Clear cached binary path (after install/uninstall)."""
    global _binary_path_cache, _binary_path_cache_set
    _binary_path_cache = None
    _binary_path_cache_set = False


def _get_search_locations() -> list:
    """(label, path) for diagnostics."""
    home = Path.home()
    return [
        ("installed", get_binary_path()),
        ("PATH", shutil.which(BINARY_NAME)),
        ("cargo", home / ".cargo" / "bin" / BINARY_NAME),
        ("/usr/local/bin", Path("/usr/local/bin") / BINARY_NAME),
        ("/usr/bin", Path("/usr/bin") / BINARY_NAME),
        ("dev-release", Path("skilllite/target/release") / BINARY_NAME),
        ("dev-debug", Path("skilllite/target/debug") / BINARY_NAME),
        ("dev-release-parent", Path("../skilllite/target/release") / BINARY_NAME),
        ("dev-debug-parent", Path("../skilllite/target/debug") / BINARY_NAME),
        ("dev-release-grandparent", Path("../../skilllite/target/release") / BINARY_NAME),
        ("dev-debug-grandparent", Path("../../skilllite/target/debug") / BINARY_NAME),
    ]


def find_binary() -> Optional[str]:
    """
    Find the skilllite binary (cached).
    Search: SKILLLITE_BINARY_PATH → cargo → ~/.skilllite/bin → PATH → system → dev builds.
    """
    global _binary_path_cache, _binary_path_cache_set
    if _binary_path_cache_set:
        return _binary_path_cache

    found: Optional[str] = None
    env_path = os.environ.get("SKILLLITE_BINARY_PATH") or os.environ.get("SKILLLITE_PATH") or os.environ.get("SKILLBOX_BINARY_PATH") or os.environ.get("SKILLBOX_PATH")
    if env_path and os.path.exists(env_path) and os.access(env_path, os.X_OK):
        found = str(Path(env_path).resolve())

    if found is None:
        cargo = Path.home() / ".cargo" / "bin" / BINARY_NAME
        if cargo.exists() and os.access(cargo, os.X_OK):
            found = str(cargo)

    if found is None:
        our = get_binary_path()
        if our.exists() and os.access(our, os.X_OK):
            found = str(our)

    if found is None:
        path_bin = shutil.which(BINARY_NAME)
        if path_bin:
            found = path_bin

    if found is None:
        for loc in [Path("/usr/local/bin") / BINARY_NAME, Path("/usr/bin") / BINARY_NAME]:
            if loc.exists() and os.access(loc, os.X_OK):
                found = str(loc)
                break

    if found is None:
        for loc in [
            Path("skilllite/target/release") / BINARY_NAME,
            Path("skilllite/target/debug") / BINARY_NAME,
            Path("../skilllite/target/release") / BINARY_NAME,
            Path("../skilllite/target/debug") / BINARY_NAME,
            Path("../../skilllite/target/release") / BINARY_NAME,
            Path("../../skilllite/target/debug") / BINARY_NAME,
        ]:
            if loc.exists() and os.access(loc, os.X_OK):
                found = str(loc.resolve())
                break

    _binary_path_cache = found
    _binary_path_cache_set = True
    return found


def check_binary_version(binary_path: str) -> Optional[str]:
    """Run --version, warn on mismatch, return version or None."""
    try:
        r = subprocess.run(
            [binary_path, "--version"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        parts = r.stdout.strip().split()
        ver = parts[-1] if parts else r.stdout.strip()
        if ver != BINARY_VERSION:
            warnings.warn(
                f"skilllite binary version mismatch: binary={ver}, SDK={BINARY_VERSION}. "
                f"Consider: skilllite install --force",
                UserWarning,
                stacklevel=2,
            )
            logger.warning("version mismatch: binary=%s, SDK=%s, path=%s", ver, BINARY_VERSION, binary_path)
        else:
            logger.debug("skilllite version OK: %s (path=%s)", ver, binary_path)
        return ver
    except Exception as e:
        logger.debug("Failed to check skilllite version: %s", e)
        return None


def install(
    version: Optional[str] = None,
    force: bool = False,
    show_progress: bool = True,
) -> Path:
    """Download and install the skilllite binary."""
    from ._install import do_install

    if not force and is_installed() and not needs_update(version):
        logger.info(f"✓ skilllite v{get_installed_version()} is already installed")
        return get_binary_path()

    dest = do_install(version=version, show_progress=show_progress)
    invalidate_binary_cache()
    return dest


def uninstall() -> bool:
    """Remove the installed binary."""
    bp = get_binary_path()
    vf = get_version_file()
    if not bp.exists():
        print("skilllite is not installed")
        return False
    bp.unlink()
    if vf.exists():
        vf.unlink()
    invalidate_binary_cache()
    print("✓ Successfully uninstalled skilllite")
    return True


def ensure_installed(
    auto_install: bool = True,
    show_progress: bool = True,
) -> str:
    """
    Ensure the skilllite binary is installed and return its path.
    Auto-installs if not found and auto_install=True.
    """
    existing = find_binary()
    if existing:
        check_binary_version(existing)
        return existing

    if auto_install:
        try:
            p = install(show_progress=show_progress)
            invalidate_binary_cache()
            return str(p)
        except Exception as e:
            raise RuntimeError(
                f"Failed to install skilllite binary: {e}\n"
                f"You can manually install: skilllite install"
            ) from e

    searched = _get_search_locations()
    paths_info = "\n".join(f"    - [{l}] {p}" for l, p in searched)
    raise FileNotFoundError(
        f"skilllite binary not found. Searched:\n{paths_info}\n\n"
        f"Install: skilllite install\n"
        f"Or build: cd skilllite && cargo build --release"
    )
