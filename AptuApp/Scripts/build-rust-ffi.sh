#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: 2026 Block, Inc.
set -euo pipefail

# Build script for Rust FFI library (aptu-ffi)
#
# Usage:
#   CI/Manual:  ./Scripts/build-rust-ffi.sh [platform] [config]
#   Xcode:      Called via environment variables (SRCROOT, CONFIGURATION, etc.)
#
# Arguments (CI/Manual mode):
#   platform: iphonesimulator (default) | iphoneos
#   config:   Debug (default) | Release

# Detect execution context
if [ -n "${SRCROOT:-}" ] && [ -n "${CONFIGURATION:-}" ]; then
    # Xcode build phase context
    echo "Running in Xcode build phase context"
    APTUAPP_DIR="$SRCROOT"
    PROJECT_ROOT="$(cd "$SRCROOT/.." && pwd)"
    CONFIG="$CONFIGURATION"
    PLATFORM="${PLATFORM_NAME:-iphonesimulator}"
    ARCH="${ARCHS:-arm64}"
else
    # CI or manual invocation
    echo "Running in CI/manual context"
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    APTUAPP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    PROJECT_ROOT="$(cd "$APTUAPP_DIR/.." && pwd)"
    PLATFORM="${1:-iphonesimulator}"
    CONFIG="${2:-Debug}"
    ARCH="arm64"
fi

echo "APTUAPP_DIR: $APTUAPP_DIR"
echo "PROJECT_ROOT: $PROJECT_ROOT"
echo "PLATFORM: $PLATFORM"
echo "CONFIG: $CONFIG"

# Determine Rust build type
if [ "$CONFIG" = "Debug" ]; then
    RUST_BUILD_TYPE=""
    RUST_PROFILE="debug"
else
    RUST_BUILD_TYPE="--release"
    RUST_PROFILE="release"
fi

# Build output directory (for compiled library)
BUILD_DIR="${APTUAPP_DIR}/build/${CONFIG}-${PLATFORM}"
mkdir -p "$BUILD_DIR"

# Generated Swift bindings directory (must exist before xcodegen)
GENERATED_DIR="${APTUAPP_DIR}/AptuApp/Generated"
mkdir -p "$GENERATED_DIR"

# Determine target triple based on platform and architecture
declare -a TARGETS=()
if [ "$PLATFORM" = "iphoneos" ]; then
    TARGETS=("aarch64-apple-ios")
elif [ "$PLATFORM" = "iphonesimulator" ]; then
    if [[ "$ARCH" == *"arm64"* ]]; then
        TARGETS+=("aarch64-apple-ios-sim")
    fi
    if [[ "$ARCH" == *"x86_64"* ]]; then
        TARGETS+=("x86_64-apple-ios")
    fi
    # Default to arm64 simulator (Apple Silicon)
    if [ ${#TARGETS[@]} -eq 0 ]; then
        TARGETS=("aarch64-apple-ios-sim")
    fi
fi

echo "Building for targets: ${TARGETS[*]}"

# Build for each target
for TARGET in "${TARGETS[@]}"; do
    echo "Building aptu-ffi for target: $TARGET"
    cd "$PROJECT_ROOT"
    
    # Ensure target is installed
    rustup target add "$TARGET" 2>/dev/null || true
    
    # Build the FFI library
    cargo build $RUST_BUILD_TYPE --target "$TARGET" -p aptu-ffi
done

# Generate Swift bindings using uniffi-bindgen
echo "Generating Swift bindings..."
cd "$PROJECT_ROOT"

FIRST_TARGET="${TARGETS[0]}"
LIB_PATH="$PROJECT_ROOT/target/${FIRST_TARGET}/${RUST_PROFILE}/libaptu_ffi.a"

if ! command -v uniffi-bindgen &> /dev/null; then
    echo "Error: uniffi-bindgen not found"
    echo "Install with: cargo install --git https://github.com/mozilla/uniffi-rs --tag v0.30.0 --bin uniffi-bindgen --features cli uniffi"
    exit 1
fi

uniffi-bindgen generate \
    --library "$LIB_PATH" \
    --language swift \
    --out-dir "$BUILD_DIR"

# Copy generated Swift bindings to AptuApp/Generated/
echo "Copying generated Swift bindings to $GENERATED_DIR"

for FILE in aptu_ffi.swift aptu_ffiFFI.h aptu_ffiFFI.modulemap; do
    if [ -f "$BUILD_DIR/$FILE" ]; then
        cp "$BUILD_DIR/$FILE" "$GENERATED_DIR/$FILE"
        echo "  Copied $FILE"
    else
        echo "  Warning: $FILE not found in $BUILD_DIR"
    fi
done

# Copy compiled library to build directory
echo "Copying compiled library to build directory..."
FIRST_TARGET="${TARGETS[0]}"
SRC_LIB="$PROJECT_ROOT/target/${FIRST_TARGET}/${RUST_PROFILE}/libaptu_ffi.a"

if [ -f "$SRC_LIB" ]; then
    cp "$SRC_LIB" "$BUILD_DIR/libaptu_ffi.a"
    echo "  Copied libaptu_ffi.a"
else
    echo "  Error: $SRC_LIB not found"
    exit 1
fi

# For multi-target builds, also copy with target-specific names
if [ ${#TARGETS[@]} -gt 1 ]; then
    for TARGET in "${TARGETS[@]}"; do
        SRC_LIB="$PROJECT_ROOT/target/${TARGET}/${RUST_PROFILE}/libaptu_ffi.a"
        if [ -f "$SRC_LIB" ]; then
            cp "$SRC_LIB" "$BUILD_DIR/libaptu_ffi_${TARGET}.a"
            echo "  Copied libaptu_ffi_${TARGET}.a"
        fi
    done
fi

echo "Rust FFI build complete!"
echo "Generated files in: $GENERATED_DIR"
echo "Library in: $BUILD_DIR"
