#!/usr/bin/env bash
# Build and install skilllite from source.
# Run from repository root. Avoids copy-paste issues with invisible Unicode chars.
set -e
cd "$(dirname "$0")/.."
cargo install --path skilllite --features memory_vector
echo "Installed: $(which skilllite)"
