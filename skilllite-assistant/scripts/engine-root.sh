# Shared helpers for locating a SkillLite engine checkout.
# shellcheck shell=bash

# Prints the engine repo root (directory that contains skilllite/Cargo.toml)
# or empty if none is found. Does not exit.
skilllite_find_engine_root() {
  local d candidate
  if [[ -n "${SKILLLITE_ENGINE_ROOT:-}" ]]; then
    candidate="$(cd "${SKILLLITE_ENGINE_ROOT}" 2>/dev/null && pwd || true)"
    if [[ -n "${candidate}" && -f "${candidate}/skilllite/Cargo.toml" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  fi

  d="${ASSISTANT_DIR:-.}"
  while [[ -n "${d}" && "${d}" != "/" ]]; do
    if [[ -f "${d}/skilllite/Cargo.toml" ]]; then
      printf '%s\n' "${d}"
      return 0
    fi
    d="$(dirname "${d}")"
  done

  # Sibling checkout: ../skilllite (engine repo next to this project)
  if [[ -f "${ASSISTANT_DIR}/../skilllite/skilllite/Cargo.toml" ]]; then
    cd "${ASSISTANT_DIR}/../skilllite" && pwd
    return 0
  fi
  return 1
}

skilllite_bin_name() {
  if [[ "${OSTYPE}" == "msys" || "${OSTYPE}" == "win32" || "${OSTYPE}" == "cygwin" ]]; then
    printf '%s\n' "skilllite.exe"
  else
    printf '%s\n' "skilllite"
  fi
}

# Resolve an existing skilllite executable (PATH, then ~/.skilllite/bin).
skilllite_find_installed_bin() {
  local name
  name="$(skilllite_bin_name)"
  if command -v "${name}" >/dev/null 2>&1; then
    command -v "${name}"
    return 0
  fi
  if [[ -x "${HOME}/.skilllite/bin/${name}" ]]; then
    printf '%s\n' "${HOME}/.skilllite/bin/${name}"
    return 0
  fi
  return 1
}

# Install `installed` into ~/.skilllite/bin without deleting the source first.
# No-op when source and destination are the same file. On copy failure the
# previous destination is left in place.
skilllite_publish_installed_bin() {
  local installed="$1"
  local bin_dir dest stage name other
  name="$(skilllite_bin_name)"
  bin_dir="${HOME}/.skilllite/bin"
  mkdir -p "${bin_dir}"
  dest="${bin_dir}/${name}"
  if [[ -e "${dest}" && "${installed}" -ef "${dest}" ]]; then
    return 0
  fi
  stage="$(mktemp "${bin_dir}/.skilllite-prebuild.XXXXXX")"
  if ! cp -f "${installed}" "${stage}"; then
    rm -f "${stage}"
    return 1
  fi
  chmod +x "${stage}"
  mv -f "${stage}" "${dest}"
  if [[ "${name}" == "skilllite" ]]; then
    other="${bin_dir}/skilllite.exe"
  else
    other="${bin_dir}/skilllite"
  fi
  rm -f "${other}"
}

# Fallback used when no engine checkout is available.
# Looks up an installed binary BEFORE touching ~/.skilllite/bin.
skilllite_publish_resolved_install() {
  local installed
  installed="$(skilllite_find_installed_bin || true)"
  if [[ -z "${installed}" ]]; then
    return 1
  fi
  skilllite_publish_installed_bin "${installed}"
}
