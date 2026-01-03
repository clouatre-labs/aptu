#!/bin/bash
set -euo pipefail

# Build script for Rust FFI library (aptu-ffi)
# Usage: build-rust-ffi.sh <SRCROOT> <CONFIGURATION> <PLATFORM_NAME> <ARCHS>

SRCROOT="${1:-.}"
CONFIGURATION="${2:-Debug}"
PLATFORM_NAME="${3:-iphoneos}"
ARCHS="${4:-arm64}"

# Determine build type
if [ "$CONFIGURATION" = "Debug" ]; then
    RUST_BUILD_TYPE=""
else
    RUST_BUILD_TYPE="--release"
fi

# Build output directory
BUILD_DIR="${SRCROOT}/build/${CONFIGURATION}-${PLATFORM_NAME}"
mkdir -p "$BUILD_DIR"

# Get the root project directory (parent of AptuApp)
PROJECT_ROOT="$(cd "$SRCROOT/.." && pwd)"

# Determine target triple based on platform and architecture
declare -a TARGETS=()
if [ "$PLATFORM_NAME" = "iphoneos" ]; then
    TARGETS=("aarch64-apple-ios")
elif [ "$PLATFORM_NAME" = "iphonesimulator" ]; then
    if [[ "$ARCHS" == *"arm64"* ]]; then
        TARGETS+=("aarch64-apple-ios-sim")
    fi
    if [[ "$ARCHS" == *"x86_64"* ]]; then
        TARGETS+=("x86_64-apple-ios")
    fi
    # Default to x86_64 if no specific arch
    if [ ${#TARGETS[@]} -eq 0 ]; then
        TARGETS=("x86_64-apple-ios")
    fi
fi

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

# Determine the library path (use first target for binding generation)
FIRST_TARGET="${TARGETS[0]}"
if [ "$CONFIGURATION" = "Debug" ]; then
    LIB_PATH="$PROJECT_ROOT/target/${FIRST_TARGET}/debug/libaptu_ffi.a"
else
    LIB_PATH="$PROJECT_ROOT/target/${FIRST_TARGET}/release/libaptu_ffi.a"
fi

# Run uniffi-bindgen to generate Swift bindings
if command -v uniffi-bindgen &> /dev/null; then
    uniffi-bindgen generate \
        "$PROJECT_ROOT/crates/aptu-ffi/src/lib.rs" \
        --language swift \
        --out-dir "$BUILD_DIR"
else
    echo "Warning: uniffi-bindgen not found, skipping Swift binding generation"
    echo "Install with: cargo install --git https://github.com/mozilla/uniffi-rs --tag v0.30.0 --bin uniffi-bindgen uniffi"
fi

# Create XCFramework from compiled libraries
echo "Creating XCFramework from compiled libraries..."
XCFRAMEWORK_PATH="$BUILD_DIR/libaptu_ffi.xcframework"
rm -rf "$XCFRAMEWORK_PATH"

# Build framework arguments for xcodebuild -create-xcframework
FRAMEWORK_ARGS=()

# Map Rust targets to platform identifiers
for TARGET in "${TARGETS[@]}"; do
    if [ "$CONFIGURATION" = "Debug" ]; then
        SRC_LIB="$PROJECT_ROOT/target/${TARGET}/debug/libaptu_ffi.a"
    else
        SRC_LIB="$PROJECT_ROOT/target/${TARGET}/release/libaptu_ffi.a"
    fi
    
    if [ -f "$SRC_LIB" ]; then
        # Determine platform identifier based on target
        if [ "$TARGET" = "aarch64-apple-ios" ]; then
            PLATFORM_ID="iphoneos"
        elif [ "$TARGET" = "aarch64-apple-ios-sim" ]; then
            PLATFORM_ID="iphonesimulator"
        elif [ "$TARGET" = "x86_64-apple-ios" ]; then
            PLATFORM_ID="iphonesimulator"
        else
            PLATFORM_ID="iphoneos"
        fi
        
        # Create temporary framework bundle for this library
        TEMP_FRAMEWORK="$BUILD_DIR/libaptu_ffi_${TARGET}.framework"
        rm -rf "$TEMP_FRAMEWORK"
        mkdir -p "$TEMP_FRAMEWORK"
        cp "$SRC_LIB" "$TEMP_FRAMEWORK/libaptu_ffi"
        
        FRAMEWORK_ARGS+=("-framework" "$TEMP_FRAMEWORK")
    fi
done

# Create XCFramework if we have frameworks to bundle
if [ ${#FRAMEWORK_ARGS[@]} -gt 0 ]; then
    xcodebuild -create-xcframework "${FRAMEWORK_ARGS[@]}" -output "$XCFRAMEWORK_PATH"
    echo "Created XCFramework at $XCFRAMEWORK_PATH"
    
    # Clean up temporary frameworks
    rm -rf "$BUILD_DIR"/libaptu_ffi_*.framework
else
    echo "Warning: No compiled libraries found for XCFramework creation"
fi

echo "Rust FFI build complete!"
