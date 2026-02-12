"""
Binary installation step for skilllite init.

Ensures the skillbox binary is installed before proceeding with project setup.
"""

import argparse

from ..sandbox.skillbox import (
    install as install_binary,
    is_installed,
    get_installed_version,
)


def run_binary_step(args: argparse.Namespace) -> None:
    """Run the binary installation step for init.

    Prints status and installs skillbox if not already installed.
    Skips if args.skip_binary is True.
    """
    if getattr(args, "skip_binary", False):
        print("\u23ed Skipping binary installation (--skip-binary)")
        return

    if is_installed():
        version = get_installed_version()
        print(f"\u2713 skillbox binary already installed (v{version})")
    else:
        print("\u2b07 Installing skillbox binary...")
        install_binary(show_progress=True)
