#!/usr/bin/env bash
# 构建前：1) 编译 skilllite 并安装到 ~/.skilllite/bin
#        2) 从安装目录复制到 resources（单文件免安装，供生产打包）
# NOTE: 使用 cargo install 产出的二进制作为唯一来源，避免 workspace release
#       profile (lto/strip/opt-level=z) 导致的兼容性问题。
set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSISTANT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$ASSISTANT_DIR/../.." && pwd)"
cd "$ROOT"

# 安装到 ~/.skilllite/bin（同时作为 resources 的来源）
mkdir -p ~/.skilllite/bin
rm -f ~/.skilllite/bin/skilllite ~/.skilllite/bin/skilllite.exe
cargo install --path skilllite --features memory_vector --root ~/.skilllite --force
echo "skilllite installed: ~/.skilllite/bin/skilllite"

# 打包到 assistant resources，供生产构建使用
RESOURCES="$ASSISTANT_DIR/src-tauri/resources"
mkdir -p "$RESOURCES"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  cp -f ~/.skilllite/bin/skilllite.exe "$RESOURCES/"
  echo "Bundled: $RESOURCES/skilllite.exe"
else
  cp -f ~/.skilllite/bin/skilllite "$RESOURCES/"
  echo "Bundled: $RESOURCES/skilllite"

  # macOS: sign the resource binary so Apple notarization succeeds
  # (Tauri only auto-signs the main binary, not files under resources/)
  if [[ "$OSTYPE" == "darwin"* ]] && [ -n "$APPLE_SIGNING_IDENTITY" ]; then
    codesign --force --sign "$APPLE_SIGNING_IDENTITY" \
      --options runtime --timestamp \
      "$RESOURCES/skilllite"
    echo "Signed resource binary: $RESOURCES/skilllite"
  fi
fi

# 内置技能：同步到 resources，供首次打开 workspace 时落地到 .skills/
BUNDLED_SKILLS="$ASSISTANT_DIR/src-tauri/resources/bundled-skills/.skills"
mkdir -p "$BUNDLED_SKILLS"
BUNDLED_SKILL_NAMES=(
  http-request
  find-skills
  skill-creator
  calculator
  text-processor
)
for name in "${BUNDLED_SKILL_NAMES[@]}"; do
  if [[ -d "$ROOT/.skills/$name" ]]; then
    rm -rf "$BUNDLED_SKILLS/$name"
    cp -R "$ROOT/.skills/$name" "$BUNDLED_SKILLS/"
    echo "Bundled skill: $BUNDLED_SKILLS/$name"
  else
    echo "WARN: missing $ROOT/.skills/$name (skip)" >&2
  fi
done
