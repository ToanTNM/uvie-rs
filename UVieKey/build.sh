#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UVIE_RS_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Building uvie-rs static library ==="
cd "$UVIE_RS_DIR"
cargo build --release

echo "=== Building UVieKey ==="
cd "$SCRIPT_DIR"

swift build \
    -Xswiftc -I"$UVIE_RS_DIR/include" \
    -Xswiftc -L"$UVIE_RS_DIR/target/release" \
    -Xlinker -luvie \
    "$@"

echo "Build complete! ($(date +%s) sec)"
echo "=== Build complete ==="
