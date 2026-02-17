#!/usr/bin/env bash
# Build skilllite Python wheel with bundled binary.
# Run from project root: ./scripts/build_wheels.sh
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST_DIR="$ROOT/skilllite"
PYTHON_SDK="$ROOT/python-sdk"

echo "==> Building Rust binary..."
cd "$RUST_DIR"
cargo build --release --bin skilllite
echo "  ✓ skilllite built"

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  BIN_NAME="skilllite.exe"
else
  BIN_NAME="skilllite"
fi

echo ""
echo "==> Building skilllite wheel..."
mkdir -p "$PYTHON_SDK/skilllite/bins"
cp "$RUST_DIR/target/release/$BIN_NAME" "$PYTHON_SDK/skilllite/bins/"
chmod +x "$PYTHON_SDK/skilllite/bins/$BIN_NAME"
cd "$PYTHON_SDK"
python -m build --wheel 2>/dev/null || pip wheel . -w dist/ --no-build-isolation
echo "  ✓ skilllite wheel in $PYTHON_SDK/dist/"

echo ""
echo "Done:"
ls -la "$PYTHON_SDK/dist/"*.whl 2>/dev/null || true
