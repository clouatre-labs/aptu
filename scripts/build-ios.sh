#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Building Aptu FFI for iOS..."

cd "$PROJECT_ROOT"

TARGETS=("aarch64-apple-ios" "aarch64-apple-ios-sim" "x86_64-apple-ios")

echo "Installing iOS targets..."
for target in "${TARGETS[@]}"; do
    rustup target add "$target" 2>/dev/null || true
done

echo "Building for iOS targets..."
for target in "${TARGETS[@]}"; do
    echo "  Building for $target..."
    cargo build -p aptu-ffi --release --target "$target"
done

echo "Creating universal binary..."
mkdir -p "$PROJECT_ROOT/target/ios-universal/release"

lipo -create \
    "$PROJECT_ROOT/target/aarch64-apple-ios/release/libaptu_ffi.a" \
    "$PROJECT_ROOT/target/aarch64-apple-ios-sim/release/libaptu_ffi.a" \
    -output "$PROJECT_ROOT/target/ios-universal/release/libaptu_ffi.a"

echo "Generating Swift bindings..."
cargo run -p aptu-ffi --bin uniffi-bindgen -- generate \
    --library "$PROJECT_ROOT/target/aarch64-apple-ios/release/libaptu_ffi.a" \
    --language swift \
    --out-dir "$PROJECT_ROOT/AptuApp/AptuApp/AptuCore/"

echo "Build complete!"
echo "  Universal library: $PROJECT_ROOT/target/ios-universal/release/libaptu_ffi.a"
echo "  Swift bindings: $PROJECT_ROOT/AptuApp/AptuApp/AptuCore/"
