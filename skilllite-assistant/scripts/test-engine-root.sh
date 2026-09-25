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

# Standalone fallback must keep ~/.skilllite/bin/skilllite when that file is
# the only installed binary. Deleting it before lookup removes the source.
unset SKILLLITE_ENGINE_ROOT || true
FALLBACK_HOME="${TMP}/home-only"
export HOME="${FALLBACK_HOME}"
export PATH="/usr/bin:/bin"
ASSISTANT_DIR="${TMP}/assistant"
mkdir -p "${HOME}/.skilllite/bin"
MARKER='preserved-skilllite-binary'
printf '%s\n' "${MARKER}" > "${HOME}/.skilllite/bin/skilllite"
chmod +x "${HOME}/.skilllite/bin/skilllite"
if ! skilllite_publish_resolved_install; then
  echo "FAIL: home-only install should satisfy the standalone fallback" >&2
  exit 1
fi
if ! grep -q "${MARKER}" "${HOME}/.skilllite/bin/skilllite"; then
  echo "FAIL: standalone fallback deleted or replaced ~/.skilllite/bin/skilllite" >&2
  exit 1
fi

# A different PATH binary replaces the destination, but a failed copy must not.
OTHER_BIN="${TMP}/other-bin"
mkdir -p "${OTHER_BIN}"
printf '%s\n' 'from-path' > "${OTHER_BIN}/skilllite"
chmod +x "${OTHER_BIN}/skilllite"
export PATH="${OTHER_BIN}:/usr/bin:/bin"
if ! skilllite_publish_resolved_install; then
  echo "FAIL: PATH binary should publish into ~/.skilllite/bin" >&2
  exit 1
fi
if ! grep -q 'from-path' "${HOME}/.skilllite/bin/skilllite"; then
  echo "FAIL: published binary does not match the PATH source" >&2
  exit 1
fi
if ! grep -q 'from-path' "${OTHER_BIN}/skilllite"; then
  echo "FAIL: publishing removed the PATH source binary" >&2
  exit 1
fi

printf '%s\n' 'keep-me' > "${HOME}/.skilllite/bin/skilllite"
if skilllite_publish_installed_bin "${TMP}/does-not-exist"; then
  echo "FAIL: missing source should not publish" >&2
  exit 1
fi
if ! grep -q 'keep-me' "${HOME}/.skilllite/bin/skilllite"; then
  echo "FAIL: failed publish removed the existing destination binary" >&2
  exit 1
fi

# Production and dev prebuild may delete ~/.skilllite/bin only after the
# engine checkout is known. The installed-binary fallback must not.
assert_rm_after_engine_lookup() {
  local file="$1"
  local lookup_line rm_line
  lookup_line="$(grep -n 'skilllite_find_engine_root' "${file}" | head -n 1 | cut -d: -f1)"
  rm_line="$(grep -n 'rm -f' "${file}" | head -n 1 | cut -d: -f1)"
  if [[ -z "${lookup_line}" || -z "${rm_line}" || "${rm_line}" -lt "${lookup_line}" ]]; then
    echo "FAIL: ${file} removes ~/.skilllite/bin before engine lookup (lookup=${lookup_line} rm=${rm_line})" >&2
    exit 1
  fi
}
assert_rm_after_engine_lookup "${SCRIPT_DIR}/prebuild-skilllite.sh"
assert_rm_after_engine_lookup "${SCRIPT_DIR}/prebuild-skilllite-dev.sh"

echo "engine-root.sh checks passed"
