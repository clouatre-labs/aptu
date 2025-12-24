#!/usr/bin/env bash

# Load bats libraries (use CI paths if available, fallback to /tmp for local)
BATS_LIB_PATH="${BATS_LIB_PATH:-/tmp}"
load "${BATS_LIB_PATH}/bats-support/load.bash"
load "${BATS_LIB_PATH}/bats-assert/load.bash"
load "${BATS_LIB_PATH}/bats-file/load.bash"

# Set APTU_BIN path to release binary
export APTU_BIN="${APTU_BIN:-$(pwd)/target/release/aptu}"

# Helper: Skip test if GitHub token not available
skip_if_no_gh_token() {
    # Check GITHUB_TOKEN first (CI environment)
    if [[ -n "$GITHUB_TOKEN" ]]; then
        return 0
    fi
    # Fall back to gh CLI, but verify it's authenticated
    if command -v gh &> /dev/null && gh auth status &> /dev/null; then
        return 0
    fi
    skip "GitHub token not available (set GITHUB_TOKEN or authenticate gh CLI)"
}

# Helper: Skip test if OpenRouter API key not available
skip_if_no_openrouter_key() {
    if [[ -z "$OPENROUTER_API_KEY" ]]; then
        skip "OpenRouter API key not available (set OPENROUTER_API_KEY)"
    fi
}
