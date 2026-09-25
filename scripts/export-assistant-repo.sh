#!/usr/bin/env bash
# Re-split the in-tree desktop directory if it still contains app sources.
# After TASK-2026-072 the engine tree is a stub — use the existing export:
#   git show-ref cursor/skilllite-assistant-export-3c6e
#   bash scripts/push-assistant-repo.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
PREFIX="skilllite-assistant"
BRANCH="export/skilllite-assistant"

if [[ ! -d "${PREFIX}" ]]; then
  echo "ERROR: ${PREFIX}/ not found" >&2
  exit 1
fi

if [[ ! -f "${PREFIX}/src-tauri/Cargo.toml" ]]; then
  echo "ERROR: ${PREFIX}/ is a stub (no src-tauri). Desktop history is already on" >&2
  echo "  origin/cursor/skilllite-assistant-export-3c6e" >&2
  echo "Create https://github.com/EXboys/skilllite-assistant then:" >&2
  echo "  bash scripts/push-assistant-repo.sh" >&2
  exit 1
fi

git subtree split --prefix="${PREFIX}" -b "${BRANCH}"
echo "Created branch ${BRANCH} containing only ${PREFIX}/"
echo "Push it to a new remote when ready:"
echo "  bash scripts/push-assistant-repo.sh"
