#!/usr/bin/env bash
set -euo pipefail

# Create a new task folder from tasks/_templates and append it to tasks/board.md.
#
# Usage:
#   bash scripts/new_task.sh "<slug>" "<Title>" [P0|P1|P2] [owner]
#
# Example:
#   bash scripts/new_task.sh "mcp-cache-lru" "Add LRU to MCP scan cache" P1 airlu

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TASKS_DIR="${ROOT_DIR}/tasks"
TEMPLATES_DIR="${TASKS_DIR}/_templates"
BOARD_FILE="${TASKS_DIR}/board.md"

SLUG="${1:-}"
TITLE="${2:-}"
PRIORITY="${3:-P1}"
OWNER="${4:-TBD}"

if [[ -z "${SLUG}" || -z "${TITLE}" ]]; then
  echo "Usage: bash scripts/new_task.sh \"<slug>\" \"<Title>\" [P0|P1|P2] [owner]"
  exit 1
fi

if [[ ! "${PRIORITY}" =~ ^P[0-2]$ ]]; then
  echo "Invalid priority: ${PRIORITY}. Allowed: P0, P1, P2."
  exit 1
fi

if [[ ! -d "${TEMPLATES_DIR}" ]]; then
  echo "Template directory not found: ${TEMPLATES_DIR}"
  exit 1
fi

if [[ ! -f "${BOARD_FILE}" ]]; then
  echo "Task board not found: ${BOARD_FILE}"
  exit 1
fi

YEAR="$(date +%Y)"
NEXT_NUM="$(
  python3 - "$TASKS_DIR" "$YEAR" <<'PY'
import re
import sys
from pathlib import Path

tasks_dir = Path(sys.argv[1])
year = sys.argv[2]
pat = re.compile(rf"^TASK-{year}-(\d{{3}})-")
nums = []
for p in tasks_dir.iterdir():
    if not p.is_dir():
        continue
    m = pat.match(p.name)
    if m:
        nums.append(int(m.group(1)))
print(f"{(max(nums) + 1) if nums else 1:03d}")
PY
)"

TASK_ID="TASK-${YEAR}-${NEXT_NUM}"
TASK_DIR_NAME="${TASK_ID}-${SLUG}"
TASK_DIR="${TASKS_DIR}/${TASK_DIR_NAME}"
TODAY="$(date +%Y-%m-%d)"

if [[ -e "${TASK_DIR}" ]]; then
  echo "Task directory already exists: ${TASK_DIR}"
  exit 1
fi

mkdir -p "${TASK_DIR}"
cp "${TEMPLATES_DIR}/TASK.md" "${TASK_DIR}/TASK.md"
cp "${TEMPLATES_DIR}/PRD.md" "${TASK_DIR}/PRD.md"
cp "${TEMPLATES_DIR}/CONTEXT.md" "${TASK_DIR}/CONTEXT.md"
cp "${TEMPLATES_DIR}/REVIEW.md" "${TASK_DIR}/REVIEW.md"
cp "${TEMPLATES_DIR}/STATUS.md" "${TASK_DIR}/STATUS.md"

python3 - "$TASK_DIR" "$TASK_ID" "$TITLE" "$PRIORITY" "$OWNER" "$TODAY" <<'PY'
import re
import sys
from pathlib import Path

task_dir = Path(sys.argv[1])
task_id, title, priority, owner, today = sys.argv[2:]
task_md = task_dir / "TASK.md"
content = task_md.read_text(encoding="utf-8")

replacements = {
    r"^- Task ID:.*$": f"- Task ID: `{task_id}`",
    r"^- Title:.*$": f"- Title: {title}",
    r"^- Status:.*$": "- Status: `draft`",
    r"^- Priority:.*$": f"- Priority: `{priority}`",
    r"^- Owner:.*$": f"- Owner: `{owner}`",
    r"^- Created:.*$": f"- Created: `{today}`",
}

for pat, rep in replacements.items():
    content = re.sub(pat, rep, content, flags=re.MULTILINE)

task_md.write_text(content, encoding="utf-8")
PY

python3 - "$BOARD_FILE" "$TASK_DIR_NAME" "$OWNER" <<'PY'
import sys
from pathlib import Path

board = Path(sys.argv[1])
task_dir_name, owner = sys.argv[2:]
line = f"- `{task_dir_name}` - Status: `draft` - Owner: `{owner}`\n"
text = board.read_text(encoding="utf-8")

ready_header = "## Ready\n"
idx = text.find(ready_header)
if idx == -1:
    text += "\n## Ready\n\n" + line
else:
    insert_pos = idx + len(ready_header)
    rest = text[insert_pos:]
    if rest.startswith("\n"):
        insert_pos += 1
    text = text[:insert_pos] + line + text[insert_pos:]

board.write_text(text, encoding="utf-8")
PY

echo "Created task: ${TASK_DIR_NAME}"
echo "Path: ${TASK_DIR}"
echo "Remember to fill PRD/CONTEXT and update STATUS as you progress."
