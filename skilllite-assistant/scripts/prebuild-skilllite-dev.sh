#!/usr/bin/env bash
# Dev-only: install skilllite to ~/.skilllite/bin when missing or forced.
# Skips bundling into src-tauri/resources (production prebuild does that).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSISTANT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
# shellcheck source=engine-root.sh
source "$SCRIPT_DIR/engine-root.sh"

BIN_DIR="${HOME}/.skilllite/bin"
BIN_NAME="$(skilllite_bin_name)"
SKILLLITE_BIN="${BIN_DIR}/${BIN_NAME}"

if [[ -x "${SKILLLITE_BIN}" && "${SKILLLITE_FORCE_PREBUILD:-}" != "1" ]]; then
  echo "dev prebuild: using ${SKILLLITE_BIN} (set SKILLLITE_FORCE_PREBUILD=1 to reinstall)"
  exit 0
fi

mkdir -p "${BIN_DIR}"
ENGINE_ROOT="$(skilllite_find_engine_root || true)"
if [[ -n "${ENGINE_ROOT}" ]]; then
  echo "dev prebuild: cargo install from ${ENGINE_ROOT}"
  rm -f "${BIN_DIR}/skilllite" "${BIN_DIR}/skilllite.exe"
  (
    cd "${ENGINE_ROOT}"
    cargo install --path skilllite --features memory_vector --root "${HOME}/.skilllite" --force
  )
else
  if ! skilllite_publish_resolved_install; then
    echo "ERROR: no engine checkout and no skilllite binary. Set SKILLLITE_ENGINE_ROOT or install skilllite." >&2
    exit 1
  fi
  echo "dev prebuild: reusing ${SKILLLITE_BIN}"
fi
echo "dev prebuild: installed ${SKILLLITE_BIN}"
