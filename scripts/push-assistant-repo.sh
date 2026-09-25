#!/usr/bin/env bash
# Push the subtree-split desktop history to github.com/EXboys/skilllite-assistant.
# Create that empty public repo first (this token cannot create it).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
REMOTE_URL="${SKILLLITE_ASSISTANT_REMOTE:-https://github.com/EXboys/skilllite-assistant.git}"
BRANCH="${1:-cursor/skilllite-assistant-export-3c6e}"

if ! git show-ref --verify --quiet "refs/heads/${BRANCH}"; then
  echo "ERROR: missing local branch ${BRANCH}. Run: bash scripts/export-assistant-repo.sh" >&2
  exit 1
fi

if ! git ls-remote "${REMOTE_URL}" HEAD >/dev/null 2>&1; then
  echo "ERROR: ${REMOTE_URL} is not reachable. Create the empty repo, then rerun." >&2
  exit 1
fi

git push -u "${REMOTE_URL}" "${BRANCH}:main"
echo "Pushed ${BRANCH} to ${REMOTE_URL} as main"
