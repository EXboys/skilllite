"""
CLI: exec bundled skilllite binary with all args.

pip install skilllite → skilllite chat/add/list/mcp/... (full CLI via binary)
"""

import os
import sys


def main() -> int:
    """Entry point: exec binary with all args. No Python subcommand parsing."""
    from .binary import get_binary

    binary = get_binary()
    if not binary:
        print("skilllite not found. Run: pip install skilllite", file=sys.stderr)
        return 1
    os.execv(binary, [binary] + sys.argv[1:])


if __name__ == "__main__":
    sys.exit(main())
