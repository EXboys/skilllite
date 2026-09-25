#!/usr/bin/env bash
# Start Vite immediately for `tauri dev`. Full release prebuild is NOT run here
# (it can take several minutes and exceeds Tauri's ~180s dev-server wait).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSISTANT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_DIR="${HOME}/.skilllite/bin"
SKILLLITE_BIN="${BIN_DIR}/skilllite"

if [[ "${SKILLLITE_SKIP_PREBUILD:-}" != "1" ]]; then
  if [[ ! -x "${SKILLLITE_BIN}" ]]; then
    echo "dev: ${SKILLLITE_BIN} not found — installing in background (log: /tmp/skilllite-prebuild-dev.log)"
    echo "dev: chat/agent features need PATH to include ~/.skilllite/bin until install finishes."
    bash "${SCRIPT_DIR}/prebuild-skilllite-dev.sh" >>/tmp/skilllite-prebuild-dev.log 2>&1 &
  elif [[ "${SKILLLITE_BACKGROUND_PREBUILD:-}" == "1" ]]; then
    echo "dev: refreshing skilllite in background (log: /tmp/skilllite-prebuild-dev.log)"
    SKILLLITE_FORCE_PREBUILD=1 bash "${SCRIPT_DIR}/prebuild-skilllite-dev.sh" >>/tmp/skilllite-prebuild-dev.log 2>&1 &
  fi
fi

DEV_URL="${TAURI_DEV_URL:-http://localhost:5173}"

port_in_use() {
  lsof -nP -iTCP:5173 -sTCP:LISTEN >/dev/null 2>&1
}

dev_server_up() {
  curl -sf --max-time 2 "${DEV_URL}/" >/dev/null 2>&1
}

cd "${ASSISTANT_DIR}"

if dev_server_up; then
  echo "dev: reusing existing Vite at ${DEV_URL} (to restart: kill \$(lsof -t -iTCP:5173 -sTCP:LISTEN))"
  # Keep beforeDevCommand alive so Tauri does not treat the hook as crashed.
  exec tail -f /dev/null
fi

if port_in_use; then
  echo "error: port 5173 is in use but ${DEV_URL} did not respond." >&2
  echo "       Another process may be stuck. Inspect:" >&2
  lsof -nP -iTCP:5173 -sTCP:LISTEN >&2 || true
  echo "       Free the port, e.g.: kill \$(lsof -t -iTCP:5173 -sTCP:LISTEN)" >&2
  exit 1
fi

exec npm run dev
