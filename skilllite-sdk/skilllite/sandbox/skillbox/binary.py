"""
Binary management for the skillbox sandbox executor.

This module handles downloading, installing, and managing the Rust-based
sandbox binary, similar to how Playwright manages browser binaries.
"""

import hashlib
import os
import platform
import shutil
import stat
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path
from typing import Optional, Tuple

# Version of the binary to download
BINARY_VERSION = "0.1.0"

# GitHub repository for releases
GITHUB_OWNER = "EXboys"
GITHUB_REPO = "skilllite"

# Base URL for downloading binaries
DOWNLOAD_BASE_URL = f"https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases/download"

# Supported platforms: (system, machine) -> platform_name
PLATFORM_MAP = {
    ("Darwin", "arm64"): "darwin-arm64",
    ("Darwin", "x86_64"): "darwin-x64",
    ("Linux", "x86_64"): "linux-x64",
    ("Linux", "aarch64"): "linux-arm64",
    ("Linux", "arm64"): "linux-arm64",
}

# Binary name per platform
BINARY_NAME = "skillbox"


def get_install_dir() -> Path:
    """
    Get the installation directory for the skillbox binary.
    
    Returns:
        Path to ~/.skillbox/bin/
    """
    return Path.home() / ".skillbox" / "bin"


def get_binary_path() -> Path:
    """
    Get the full path to the installed binary.
    
    Returns:
        Path to ~/.skillbox/bin/skillbox
    """
    return get_install_dir() / BINARY_NAME


def get_version_file() -> Path:
    """
    Get the path to the version file.
    
    Returns:
        Path to ~/.skillbox/.version
    """
    return Path.home() / ".skillbox" / ".version"


def get_platform() -> str:
    """
    Detect the current platform.
    
    Returns:
        Platform string like 'darwin-arm64', 'linux-x64', etc.
        
    Raises:
        RuntimeError: If the platform is not supported.
    """
    system = platform.system()
    machine = platform.machine()
    
    key = (system, machine)
    if key not in PLATFORM_MAP:
        raise RuntimeError(
            f"Unsupported platform: {system} {machine}. "
            f"Supported platforms: macOS (x64, arm64), Linux (x64, arm64)"
        )
    
    return PLATFORM_MAP[key]


def get_download_url(version: Optional[str] = None) -> str:
    """
    Get the download URL for the current platform.
    
    Args:
        version: Version to download. Defaults to BINARY_VERSION.
        
    Returns:
        Full download URL for the binary.
    """
    version = version or BINARY_VERSION
    plat = get_platform()
    
    # Binary naming convention: skillbox-{platform}.tar.gz
    filename = f"skillbox-{plat}.tar.gz"
    
    return f"{DOWNLOAD_BASE_URL}/v{version}/{filename}"


def is_installed() -> bool:
    """
    Check if the skillbox binary is installed.
    
    Returns:
        True if the binary exists and is executable.
    """
    binary_path = get_binary_path()
    return binary_path.exists() and os.access(binary_path, os.X_OK)


def get_installed_version() -> Optional[str]:
    """
    Get the version of the installed binary.
    
    Returns:
        Version string, or None if not installed or version unknown.
    """
    version_file = get_version_file()
    if version_file.exists():
        return version_file.read_text().strip()
    return None


def needs_update(target_version: Optional[str] = None) -> bool:
    """
    Check if the binary needs to be updated.
    
    Args:
        target_version: Target version to check against.
        
    Returns:
        True if update is needed.
    """
    if not is_installed():
        return True
    
    target_version = target_version or BINARY_VERSION
    installed_version = get_installed_version()
    
    if installed_version is None:
        return True
    
    return installed_version != target_version


def download_with_progress(url: str, dest: Path, show_progress: bool = True) -> None:
    """
    Download a file with optional progress display.
    
    Args:
        url: URL to download from.
        dest: Destination path.
        show_progress: Whether to show progress bar.
    """
    def report_progress(block_num: int, block_size: int, total_size: int) -> None:
        if not show_progress or total_size <= 0:
            return
        
        downloaded = block_num * block_size
        percent = min(100, downloaded * 100 // total_size)
        bar_length = 40
        filled = int(bar_length * percent // 100)
        bar = "█" * filled + "░" * (bar_length - filled)
        
        sys.stdout.write(f"\r  Downloading: [{bar}] {percent}%")
        sys.stdout.flush()
        
        if downloaded >= total_size:
            sys.stdout.write("\n")
            sys.stdout.flush()
    
    try:
        urllib.request.urlretrieve(url, dest, reporthook=report_progress if show_progress else None)
    except urllib.error.HTTPError as e:
        if e.code == 404:
            raise RuntimeError(
                f"Binary not found at {url}. "
                f"Please check if version {BINARY_VERSION} has been released."
            ) from e
        raise


def extract_archive(archive_path: Path, dest_dir: Path) -> Path:
    """
    Extract a tar.gz or zip archive.
    
    Args:
        archive_path: Path to the archive.
        dest_dir: Directory to extract to.
        
    Returns:
        Path to the extracted binary.
    """
    if archive_path.suffix == ".gz" or str(archive_path).endswith(".tar.gz"):
        with tarfile.open(archive_path, "r:gz") as tar:
            tar.extractall(dest_dir)
    elif archive_path.suffix == ".zip":
        with zipfile.ZipFile(archive_path, "r") as zip_ref:
            zip_ref.extractall(dest_dir)
    else:
        raise RuntimeError(f"Unknown archive format: {archive_path}")
    
    # Find the binary in extracted files
    binary_path = dest_dir / BINARY_NAME
    if binary_path.exists():
        return binary_path
    
    # Check if it's in a subdirectory
    for item in dest_dir.iterdir():
        if item.is_dir():
            nested_binary = item / BINARY_NAME
            if nested_binary.exists():
                return nested_binary
    
    raise RuntimeError(f"Binary not found in archive: {archive_path}")


def install(
    version: Optional[str] = None,
    force: bool = False,
    show_progress: bool = True
) -> Path:
    """
    Download and install the skillbox binary.
    
    Args:
        version: Version to install. Defaults to BINARY_VERSION.
        force: Force reinstall even if already installed.
        show_progress: Show download progress.
        
    Returns:
        Path to the installed binary.
    """
    version = version or BINARY_VERSION
    
    if not force and is_installed() and not needs_update(version):
        installed_version = get_installed_version()
        print(f"✓ skillbox v{installed_version} is already installed")
        return get_binary_path()
    
    plat = get_platform()
    print(f"Installing skillbox v{version} for {plat}...")
    
    # Create install directory
    install_dir = get_install_dir()
    install_dir.mkdir(parents=True, exist_ok=True)
    
    # Download to temp directory
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_path = Path(temp_dir)
        archive_name = f"skillbox-{plat}.tar.gz"
        archive_path = temp_path / archive_name
        
        # Download
        url = get_download_url(version)
        print(f"  Downloading from: {url}")
        download_with_progress(url, archive_path, show_progress)
        
        # Extract
        print("  Extracting...")
        extracted_binary = extract_archive(archive_path, temp_path)
        
        # Move to install location
        dest_binary = get_binary_path()
        if dest_binary.exists():
            dest_binary.unlink()
        
        shutil.move(str(extracted_binary), str(dest_binary))
        
        # Make executable
        dest_binary.chmod(dest_binary.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    
    # Write version file
    version_file = get_version_file()
    version_file.parent.mkdir(parents=True, exist_ok=True)
    version_file.write_text(version)
    
    print(f"✓ Successfully installed skillbox v{version}")
    print(f"  Location: {dest_binary}")
    
    return dest_binary


def uninstall() -> bool:
    """
    Uninstall the skillbox binary.
    
    Returns:
        True if uninstalled, False if not installed.
    """
    binary_path = get_binary_path()
    version_file = get_version_file()
    
    if not binary_path.exists():
        print("skillbox is not installed")
        return False
    
    binary_path.unlink()
    if version_file.exists():
        version_file.unlink()
    
    print("✓ Successfully uninstalled skillbox")
    return True


def find_binary() -> Optional[str]:
    """
    Find the skillbox binary.
    
    Search order:
    1. ~/.skillbox/bin/skillbox (installed by this package)
    2. System PATH
    3. ~/.cargo/bin/skillbox (cargo install)
    4. Common system locations
    5. Development build locations
    
    Returns:
        Path to the binary, or None if not found.
    """
    # 1. Check our install location first
    our_binary = get_binary_path()
    if our_binary.exists() and os.access(our_binary, os.X_OK):
        return str(our_binary)
    
    # 2. Check PATH
    path_binary = shutil.which(BINARY_NAME)
    if path_binary:
        return path_binary
    
    # 3. Check cargo install location
    cargo_binary = Path.home() / ".cargo" / "bin" / BINARY_NAME
    if cargo_binary.exists():
        return str(cargo_binary)
    
    # 4. Check common system locations
    system_locations = [
        Path("/usr/local/bin") / BINARY_NAME,
        Path("/usr/bin") / BINARY_NAME,
    ]
    
    for loc in system_locations:
        if loc.exists() and os.access(loc, os.X_OK):
            return str(loc)
    
    # 5. Check development build locations (relative to common project structures)
    dev_locations = [
        Path("skillbox/target/release") / BINARY_NAME,
        Path("../skillbox/target/release") / BINARY_NAME,
        Path("../../skillbox/target/release") / BINARY_NAME,
    ]
    
    for loc in dev_locations:
        if loc.exists() and os.access(loc, os.X_OK):
            return str(loc.resolve())
    
    return None


def ensure_installed(
    auto_install: bool = True,
    show_progress: bool = True
) -> str:
    """
    Ensure the skillbox binary is installed and return its path.
    
    This is the main entry point for getting a working binary path.
    It will:
    1. Try to find an existing binary
    2. If not found and auto_install is True, download and install it
    3. Raise an error if binary cannot be found or installed
    
    Args:
        auto_install: Automatically install if not found.
        show_progress: Show download progress during installation.
        
    Returns:
        Path to the binary.
        
    Raises:
        FileNotFoundError: If binary not found and auto_install is False.
        RuntimeError: If installation fails.
    """
    # First, try to find existing binary
    existing = find_binary()
    if existing:
        return existing
    
    # Not found - try to install if allowed
    if auto_install:
        try:
            installed_path = install(show_progress=show_progress)
            return str(installed_path)
        except Exception as e:
            raise RuntimeError(
                f"Failed to install skillbox binary: {e}\n"
                f"You can manually install it with: skilllite install"
            ) from e
    
    # Not found and not allowed to install
    raise FileNotFoundError(
        "skillbox binary not found. Install it with:\n"
        "  skilllite install\n"
        "Or build from source:\n"
        "  cd skillbox && cargo build --release"
    )
