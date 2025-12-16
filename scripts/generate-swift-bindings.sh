#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Generating Swift bindings from Rust FFI..."

cd "$PROJECT_ROOT"

LIBRARY_PATH="${1:-$PROJECT_ROOT/target/aarch64-apple-ios/release/libaptu_ffi.a}"
OUTPUT_DIR="${2:-$PROJECT_ROOT/AptuApp/AptuApp/AptuCore}"

if [ ! -f "$LIBRARY_PATH" ]; then
    echo "Error: Library not found at $LIBRARY_PATH"
    echo "Run ./scripts/build-ios.sh first"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "Generating bindings from: $LIBRARY_PATH"
echo "Output directory: $OUTPUT_DIR"

cargo run -p aptu-ffi --bin uniffi-bindgen -- generate \
    --library "$LIBRARY_PATH" \
    --language swift \
    --out-dir "$OUTPUT_DIR"

echo "Swift bindings generated successfully!"
echo "Generated files:"
ls -la "$OUTPUT_DIR"
