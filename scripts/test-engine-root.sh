#!/usr/bin/env bash
# Falsifiable checks for engine-root discovery.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSISTANT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
# shellcheck source=engine-root.sh
source "$SCRIPT_DIR/engine-root.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Isolated tree with no engine checkout must fail when env is unset.
unset SKILLLITE_ENGINE_ROOT || true
ASSISTANT_DIR="${TMP}/assistant"
mkdir -p "${ASSISTANT_DIR}"
if skilllite_find_engine_root >/dev/null 2>&1; then
  echo "FAIL: isolated assistant dir should not find an engine root" >&2
  exit 1
fi

# Parent checkout with skilllite/Cargo.toml is detected.
ENGINE="${TMP}/engine"
mkdir -p "${ENGINE}/skilllite" "${ENGINE}/desktop"
printf '%s\n' '[package]' > "${ENGINE}/skilllite/Cargo.toml"
ASSISTANT_DIR="${ENGINE}/desktop"
FOUND="$(skilllite_find_engine_root)"
if [[ "${FOUND}" != "${ENGINE}" ]]; then
  echo "FAIL: expected ${ENGINE}, got ${FOUND}" >&2
  exit 1
fi

# Explicit env wins.
export SKILLLITE_ENGINE_ROOT="${ENGINE}"
ASSISTANT_DIR="${TMP}/elsewhere"
mkdir -p "${ASSISTANT_DIR}"
FOUND="$(skilllite_find_engine_root)"
if [[ "${FOUND}" != "${ENGINE}" ]]; then
  echo "FAIL: SKILLLITE_ENGINE_ROOT not honored: ${FOUND}" >&2
  exit 1
fi

echo "engine-root.sh checks passed"
