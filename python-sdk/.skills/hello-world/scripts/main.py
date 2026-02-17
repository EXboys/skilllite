#!/usr/bin/env python3
"""Hello-world skill entry point."""
import json
import sys


def main():
    data = json.loads(sys.stdin.read())
    name = data.get("name", "World")
    result = {"greeting": f"Hello, {name}!"}
    print(json.dumps(result))


if __name__ == "__main__":
    main()
