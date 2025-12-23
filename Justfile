# Default recipe
default:
    @just --list

# Check code: format, lint, and test
check: fmt lint test
    @echo "All checks passed!"

# Check formatting
fmt:
    cargo fmt --check

# Fix formatting
fmt-fix:
    cargo fmt

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Fix clippy issues (where possible)
lint-fix:
    cargo clippy --fix --allow-dirty --allow-staged

# Run unit tests
test:
    cargo test --lib

# Run integration tests (requires bats and release binary)
integration: build-release
    APTU_BIN=./target/release/aptu bats tests/integration.bats

# Build debug binary
build:
    cargo build

# Build release binary
build-release:
    cargo build --release

# Clean build artifacts
clean:
    cargo clean

# Run the CLI (requires arguments)
run *ARGS:
    cargo run -- {{ARGS}}

# Run full CI pipeline locally
ci: fmt lint test build
    @echo "CI pipeline complete!"

# Check REUSE compliance (license headers)
reuse:
    uv tool run reuse lint

# Install binary locally
install:
    cargo install --path crates/aptu-cli
