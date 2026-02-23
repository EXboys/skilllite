#!/usr/bin/env bash
# 构建前：1) 编译 skilllite 并打包到 resources（单文件免安装）
#        2) 同时安装到 ~/.skilllite/bin（供 tauri dev 使用）
set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ASSISTANT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$ASSISTANT_DIR/../.." && pwd)"
cd "$ROOT"

# 编译 skilllite（含 memory_vector）
cargo build -p skilllite --release --features memory_vector

# 打包到 assistant resources，供生产构建使用
RESOURCES="$ASSISTANT_DIR/src-tauri/resources"
mkdir -p "$RESOURCES"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  cp -f target/release/skilllite.exe "$RESOURCES/"
  echo "Bundled: $RESOURCES/skilllite.exe"
else
  cp -f target/release/skilllite "$RESOURCES/"
  echo "Bundled: $RESOURCES/skilllite"
fi

# 安装到 ~/.skilllite/bin（供 tauri dev 使用）
mkdir -p ~/.skilllite/bin
rm -f ~/.skilllite/bin/skilllite ~/.skilllite/bin/skilllite.exe
cargo install --path skilllite --features memory_vector --root ~/.skilllite --force
echo "skilllite installed: ~/.skilllite/bin/skilllite"
