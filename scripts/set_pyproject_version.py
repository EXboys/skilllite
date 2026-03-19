#!/usr/bin/env python3
"""Set version in python-sdk/pyproject.toml from env VERSION (e.g. from tag). Used by CI."""
import os
import re

def main():
    p = os.environ.get("PYPROJECT_PATH", "python-sdk/pyproject.toml")
    v = os.environ.get("VERSION", "0.0.0").lstrip("v")
    with open(p, "r", encoding="utf-8") as f:
        s = f.read()
    s = re.sub(r'^version = ".*"', f'version = "{v}"', s, count=1, flags=re.M)
    with open(p, "w", encoding="utf-8") as f:
        f.write(s)
    print(f"Set version to {v} in {p}")

if __name__ == "__main__":
    main()
