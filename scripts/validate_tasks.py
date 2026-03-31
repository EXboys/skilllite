#!/usr/bin/env python3
"""Validate task-centered workflow artifacts."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TASKS_DIR = ROOT / "tasks"
BOARD_FILE = TASKS_DIR / "board.md"
REQUIRED_TASK_FILES = {"TASK.md", "PRD.md", "CONTEXT.md", "REVIEW.md", "STATUS.md"}
ALLOWED_STATUSES = {"draft", "ready", "in_progress", "in_review", "done", "blocked", "cancelled"}


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def validate_task_dir(task_dir: Path, errors: list[str]) -> None:
    missing = [f for f in sorted(REQUIRED_TASK_FILES) if not (task_dir / f).exists()]
    if missing:
        errors.append(f"{task_dir.name}: missing files -> {', '.join(missing)}")
        return

    task_md = read_text(task_dir / "TASK.md")
    status_md = read_text(task_dir / "STATUS.md")
    review_md = read_text(task_dir / "REVIEW.md")

    for field in ("Task ID:", "Title:", "Status:", "Priority:", "Owner:", "Created:"):
        if field not in task_md:
            errors.append(f"{task_dir.name}: TASK.md missing metadata field '{field}'")

    status_match = re.search(r"^- Status:\s*`([^`]+)`", task_md, flags=re.MULTILINE)
    if not status_match:
        errors.append(f"{task_dir.name}: TASK.md status is missing or not backtick-quoted")
    else:
        status = status_match.group(1).strip()
        if status not in ALLOWED_STATUSES:
            errors.append(
                f"{task_dir.name}: invalid status '{status}' in TASK.md "
                f"(allowed: {', '.join(sorted(ALLOWED_STATUSES))})"
            )

    if "## Timeline" not in status_md:
        errors.append(f"{task_dir.name}: STATUS.md missing '## Timeline'")
    if "## Checkpoints" not in status_md:
        errors.append(f"{task_dir.name}: STATUS.md missing '## Checkpoints'")
    if "Merge readiness:" not in review_md:
        errors.append(f"{task_dir.name}: REVIEW.md missing 'Merge readiness'")


def main() -> int:
    errors: list[str] = []

    if not TASKS_DIR.exists():
        print("tasks/ not found; skipping validation.")
        return 0

    if not BOARD_FILE.exists():
        errors.append("tasks/board.md not found")
        board_text = ""
    else:
        board_text = read_text(BOARD_FILE)

    task_dirs = sorted(
        [p for p in TASKS_DIR.iterdir() if p.is_dir() and p.name.startswith("TASK-")],
        key=lambda p: p.name,
    )

    for task_dir in task_dirs:
        validate_task_dir(task_dir, errors)
        if board_text and task_dir.name not in board_text:
            errors.append(f"{task_dir.name}: not listed in tasks/board.md")

    if errors:
        print("Task validation failed:")
        for e in errors:
            print(f"- {e}")
        return 1

    print(f"Task validation passed ({len(task_dirs)} task directories checked).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
