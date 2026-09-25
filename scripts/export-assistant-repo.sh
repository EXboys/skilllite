#!/usr/bin/env bash
# Export the standalone desktop project as a subtree branch for a future remote.
# Usage:
#   bash scripts/export-assistant-repo.sh
# Then (after creating the empty GitHub repo):
#   git push git@github.com:EXboys/skilllite-assistant.git "export/skilllite-assistant:main"
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
PREFIX="skilllite-assistant"
BRANCH="export/skilllite-assistant"

if [[ ! -d "${PREFIX}" ]]; then
  echo "ERROR: ${PREFIX}/ not found" >&2
  exit 1
fi

git subtree split --prefix="${PREFIX}" -b "${BRANCH}"
echo "Created branch ${BRANCH} containing only ${PREFIX}/"
echo "Push it to a new remote when ready:"
echo "  git push <assistant-remote> ${BRANCH}:main"
