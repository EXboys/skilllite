#!/usr/bin/env bash
# Build-time: install or copy skilllite into ~/.skilllite/bin and src-tauri/resources.
# Engine source is optional. Prefer SKILLLITE_ENGINE_ROOT or a parent/sibling checkout;
# otherwise reuse an already-installed binary.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSISTANT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
# shellcheck source=engine-root.sh
source "$SCRIPT_DIR/engine-root.sh"

BIN_NAME="$(skilllite_bin_name)"
mkdir -p "${HOME}/.skilllite/bin"
rm -f "${HOME}/.skilllite/bin/skilllite" "${HOME}/.skilllite/bin/skilllite.exe"

ENGINE_ROOT="$(skilllite_find_engine_root || true)"
if [[ -n "${ENGINE_ROOT}" ]]; then
  echo "prebuild: cargo install from engine checkout ${ENGINE_ROOT}"
  (
    cd "${ENGINE_ROOT}"
    cargo install --path skilllite --features memory_vector --root "${HOME}/.skilllite" --force
  )
else
  INSTALLED="$(skilllite_find_installed_bin || true)"
  if [[ -z "${INSTALLED}" ]]; then
    echo "ERROR: no SkillLite engine checkout and no skilllite on PATH." >&2
    echo "Install the engine (pip install skilllite / cargo install skilllite)" >&2
    echo "or set SKILLLITE_ENGINE_ROOT to a skilllite repo checkout." >&2
    exit 1
  fi
  echo "prebuild: copying installed binary ${INSTALLED}"
  cp -f "${INSTALLED}" "${HOME}/.skilllite/bin/${BIN_NAME}"
fi

echo "skilllite installed: ${HOME}/.skilllite/bin/${BIN_NAME}"

RESOURCES="${ASSISTANT_DIR}/src-tauri/resources"
mkdir -p "${RESOURCES}"
cp -f "${HOME}/.skilllite/bin/${BIN_NAME}" "${RESOURCES}/"
echo "Bundled: ${RESOURCES}/${BIN_NAME}"

if [[ "${OSTYPE}" == "darwin"* && -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --sign "${APPLE_SIGNING_IDENTITY}" \
    --options runtime --timestamp \
    "${RESOURCES}/${BIN_NAME}"
  echo "Signed resource binary: ${RESOURCES}/${BIN_NAME}"
fi

# Refresh bundled skills from the engine tree when present; otherwise keep vendored copies.
BUNDLED_SKILLS="${ASSISTANT_DIR}/src-tauri/resources/bundled-skills/.skills"
mkdir -p "${BUNDLED_SKILLS}"
BUNDLED_SKILL_NAMES=(
  http-request
  find-skills
  skill-creator
  calculator
  text-processor
)
if [[ -n "${ENGINE_ROOT}" ]]; then
  for name in "${BUNDLED_SKILL_NAMES[@]}"; do
    if [[ -d "${ENGINE_ROOT}/.skills/${name}" ]]; then
      rm -rf "${BUNDLED_SKILLS}/${name}"
      cp -R "${ENGINE_ROOT}/.skills/${name}" "${BUNDLED_SKILLS}/"
      echo "Bundled skill: ${BUNDLED_SKILLS}/${name}"
    else
      echo "WARN: missing ${ENGINE_ROOT}/.skills/${name} (keep vendored copy if any)" >&2
    fi
  done
else
  echo "prebuild: no engine checkout; using vendored bundled-skills"
fi
