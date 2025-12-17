#!/usr/bin/env bats

setup_file() {
    export APTU_BIN="${APTU_BIN:-$(pwd)/target/release/aptu}"
    if [[ ! -f "$APTU_BIN" ]]; then
        echo "Error: APTU_BIN not found at $APTU_BIN" >&2
        exit 1
    fi
}

load test_helper

@test "auth status with gh CLI fallback" {
    skip_if_no_gh_token
    run "$APTU_BIN" auth status
    assert_success
}

@test "repo list JSON validity via jq" {
    run "$APTU_BIN" repo list --output json
    assert_success
    
    # Verify output is valid JSON
    echo "$output" | jq . > /dev/null
    assert_success
}

@test "issue list with real GitHub API" {
    # Workaround: aptu doesn't support GITHUB_TOKEN env var yet (pre-existing bug)
    # Skip in CI if gh CLI not available, since aptu requires interactive auth
    if [[ -n "$CI" ]] && ! command -v gh &> /dev/null; then
        skip "Requires gh CLI in CI (aptu doesn't support GITHUB_TOKEN env var yet)"
    fi
    
    # Ensure GITHUB_TOKEN is set from gh CLI if not already set
    if [[ -z "$GITHUB_TOKEN" ]]; then
        if command -v gh &> /dev/null; then
            token=$(gh auth token 2>/dev/null) || skip "GitHub token not available (gh CLI not authenticated)"
            export GITHUB_TOKEN="$token"
        else
            skip "GitHub token not available (set GITHUB_TOKEN or install gh CLI)"
        fi
    fi
    run "$APTU_BIN" issue list block/goose
    assert_success
}

@test "issue triage --dry-run with OpenRouter API" {
    skip_if_no_gh_token
    skip_if_no_openrouter_key
    
    # Use a real issue URL for testing
    run "$APTU_BIN" issue triage https://github.com/block/goose/issues/1 --dry-run
    assert_success
}

@test "repo list output is valid JSON array" {
    run "$APTU_BIN" repo list --output json
    assert_success
    
    # Parse and verify it's an array
    result=$(echo "$output" | jq 'type')
    [[ "$result" == '"array"' ]]
}

@test "history returns valid JSON" {
    run "$APTU_BIN" history --output json
    assert_success
    
    # Verify output is valid JSON
    echo "$output" | jq . > /dev/null
    assert_success
}
