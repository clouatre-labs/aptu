#!/usr/bin/env bats

load test_helper

setup() {
    SCRIPT="$(pwd)/scripts/telemetry-rollup.sh"
    WORK_DIR="$(mktemp -d)"
    CONTEXT_FILE="$WORK_DIR/context.jsonl"
    DEST_FILE="$WORK_DIR/out/rollup.jsonl"
}

teardown() {
    rm -rf "$WORK_DIR"
}

write_fixture() {
    printf '%s\n' \
        '{"trace_id":"t1","operation":"pr_review","pr":"owner/repo#1","model":"gpt-4","github_actor":"alice","files_total":10,"files_with_patch":5,"files_truncated":2,"truncated_chars_dropped":100,"ast_context_chars":0,"call_graph_chars":0,"dep_enrichments_count":0,"dep_enrichments_chars":0,"budget_drops":["call_graph"],"cwd_inferred":false,"prompt_chars_final":90000,"finish_reasons":["stop"],"max_prompt_chars":120000}' \
        '{"trace_id":"t2","operation":"pr_review","pr":"owner/repo#2","model":"gpt-4","github_actor":"bob","files_total":3,"files_with_patch":3,"files_truncated":0,"truncated_chars_dropped":0,"ast_context_chars":0,"call_graph_chars":0,"dep_enrichments_count":0,"dep_enrichments_chars":0,"budget_drops":[],"cwd_inferred":false,"prompt_chars_final":30000,"finish_reasons":["stop"],"max_prompt_chars":120000}' \
        '{"trace_id":"t3","operation":"pr_review","pr":"owner/repo#3","model":"claude-3","github_actor":"carol","files_total":1,"files_with_patch":1,"files_truncated":1,"truncated_chars_dropped":5,"ast_context_chars":0,"call_graph_chars":0,"dep_enrichments_count":0,"dep_enrichments_chars":0,"budget_drops":["full_content","call_graph"],"cwd_inferred":false,"prompt_chars_final":115000,"finish_reasons":["stop"],"max_prompt_chars":120000}' \
        > "$CONTEXT_FILE"
}

@test "missing context file: exits 0 and creates no destination" {
    run "$SCRIPT" "$WORK_DIR/does-not-exist.jsonl" "$DEST_FILE"
    assert_success
    assert_file_not_exists "$DEST_FILE"
}

@test "unset context file: exits 0 and creates no destination" {
    run "$SCRIPT" "" "$DEST_FILE"
    assert_success
    assert_file_not_exists "$DEST_FILE"
}

@test "fixture JSONL: computes correct counters and appends one compact JSON line" {
    write_fixture

    run "$SCRIPT" "$CONTEXT_FILE" "$DEST_FILE"
    assert_success
    assert_file_exists "$DEST_FILE"

    [ "$(wc -l <"$DEST_FILE")" -eq 1 ]

    run jq -c '.reviews_total' "$DEST_FILE"
    assert_output "3"

    run jq -c '.truncation_events_total' "$DEST_FILE"
    assert_output "2"

    run jq -c '.files_truncated_total' "$DEST_FILE"
    assert_output "3"

    run jq -Sc '.budget_drop_reason_counts' "$DEST_FILE"
    assert_output '{"call_graph":2,"full_content":1}'

    run jq -Sc '.model_tier_counts' "$DEST_FILE"
    assert_output '{"claude-3":1,"gpt-4":2}'

    run jq -Sc '.prompt_budget_pct_histogram' "$DEST_FILE"
    assert_output '{"bucket_counts":[1,0,1,0,1],"explicit_bounds":[25,50,75,90]}'

    run jq -Sc '.finish_reasons_counts' "$DEST_FILE"
    assert_output '{"stop":3}'
}

@test "fixture JSONL with pr and github_actor: appended line contains neither key" {
    write_fixture

    run "$SCRIPT" "$CONTEXT_FILE" "$DEST_FILE"
    assert_success

    run jq -e 'has("pr")' "$DEST_FILE"
    assert_output "false"

    run jq -e 'has("github_actor")' "$DEST_FILE"
    assert_output "false"
}

@test "malformed JSONL: does not exit nonzero and destination stays absent" {
    printf 'not json\n' > "$CONTEXT_FILE"

    run "$SCRIPT" "$CONTEXT_FILE" "$DEST_FILE"
    assert_success
    assert_file_not_exists "$DEST_FILE"
}

@test "empty JSONL: does not exit nonzero" {
    : > "$CONTEXT_FILE"

    run "$SCRIPT" "$CONTEXT_FILE" "$DEST_FILE"
    assert_success
}
