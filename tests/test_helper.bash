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
    if [[ -z "$GITHUB_TOKEN" ]] && ! command -v gh &> /dev/null; then
        skip "GitHub token not available (set GITHUB_TOKEN or install gh CLI)"
    fi
}

# Helper: Skip test if OpenRouter API key not available
skip_if_no_openrouter_key() {
    if [[ -z "$OPENROUTER_API_KEY" ]]; then
        skip "OpenRouter API key not available (set OPENROUTER_API_KEY)"
    fi
}

# Helper: Get GitHub token from gh CLI or environment
get_github_token() {
    if [[ -n "$GITHUB_TOKEN" ]]; then
        echo "$GITHUB_TOKEN"
    elif command -v gh &> /dev/null; then
        gh auth token
    else
        echo ""
    fi
}
