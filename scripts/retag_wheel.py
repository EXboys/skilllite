#!/usr/bin/env python3
"""Retag wheels in a directory with a platform tag. Used by CI to produce
platform-specific wheel filenames that don't collide when merged."""
import glob
import subprocess
import sys

def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <dist_dir> <platform_tag>")
        sys.exit(1)
    dist_dir, plat_tag = sys.argv[1], sys.argv[2]
    wheels = glob.glob(f"{dist_dir}/*.whl")
    if not wheels:
        print(f"ERROR: no .whl files found in {dist_dir}")
        sys.exit(1)
    for whl in wheels:
        subprocess.check_call([
            sys.executable, "-m", "wheel", "tags",
            f"--platform-tag={plat_tag}", "--remove", whl,
        ])
    print(f"Retagged {len(wheels)} wheel(s) with platform={plat_tag}")

if __name__ == "__main__":
    main()
