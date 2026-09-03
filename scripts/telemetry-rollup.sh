#!/usr/bin/env bash
# Roll up anonymized, aggregate-only counters from a single review-context JSONL
# file (APTU_CONTEXT_FILE) and append them as one compact JSON line to an
# operator-controlled destination path.
#
# Args:
#   $1  context_file  path to the APTU_CONTEXT_FILE JSONL (may be unset/empty)
#   $2  destination    path to append the rollup JSON line to
#
# This script never fails the caller: any internal error (missing jq,
# malformed JSONL, unwritable destination) is logged to stderr and the script
# still exits 0.

set -u

main() {
    local context_file="${1:-}"
    local destination="${2:-}"

    if [ -z "$context_file" ] || [ ! -f "$context_file" ]; then
        return 0
    fi

    if [ -z "$destination" ]; then
        echo "telemetry-rollup: destination path is empty, skipping" >&2
        return 0
    fi

    if ! command -v jq >/dev/null 2>&1; then
        echo "telemetry-rollup: jq not found, skipping" >&2
        return 0
    fi

    local jq_err rollup
    jq_err=$(mktemp)
    if ! rollup=$(jq -sc '
        {
            reviews_total: length,
            truncation_events_total: (
                [.[] | select((.files_truncated // 0) > 0 or ((.budget_drops // []) | length) > 0)]
                | length
            ),
            files_truncated_total: ([.[] | .files_truncated // 0] | add // 0),
            budget_drop_reason_counts: (
                [.[] | .budget_drops // [] | .[]]
                | reduce .[] as $reason ({}; .[$reason] = ((.[$reason] // 0) + 1))
            ),
            finish_reasons_counts: (
                [.[] | .finish_reasons // [] | .[]]
                | reduce .[] as $reason ({}; .[$reason] = ((.[$reason] // 0) + 1))
            ),
            model_tier_counts: (
                [.[] | .model // "unknown"]
                | reduce .[] as $model ({}; .[$model] = ((.[$model] // 0) + 1))
            ),
            prompt_budget_pct_histogram: (
                [.[] | (
                    (((.prompt_chars_final // 0) * 100) / (if (.max_prompt_chars // 0) > 0 then .max_prompt_chars else 120000 end)) | round
                ) | (
                    if . <= 25 then 0
                    elif . <= 50 then 1
                    elif . <= 75 then 2
                    elif . <= 90 then 3
                    else 4
                    end
                )]
                | reduce .[] as $idx ([0,0,0,0,0]; .[$idx] += 1)
                | {explicit_bounds: [25,50,75,90], bucket_counts: .}
            )
        }
    ' "$context_file" 2>"$jq_err"); then
        echo "telemetry-rollup: jq failed to compute counters: $(cat "$jq_err" 2>/dev/null)" >&2
        rm -f "$jq_err"
        return 0
    fi
    rm -f "$jq_err"

    local run_id timestamp
    run_id="${GITHUB_RUN_ID:-unknown}"
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    local line
    jq_err=$(mktemp)
    if ! line=$(jq -nc \
        --argjson rollup "$rollup" \
        --arg run_id "$run_id" \
        --arg timestamp "$timestamp" \
        '$rollup + {run_id: $run_id, timestamp: $timestamp}' 2>"$jq_err"); then
        echo "telemetry-rollup: jq failed to build output line: $(cat "$jq_err" 2>/dev/null)" >&2
        rm -f "$jq_err"
        return 0
    fi
    rm -f "$jq_err"

    local dest_dir mkdir_err
    dest_dir=$(dirname -- "$destination")
    mkdir_err=$(mktemp)
    if ! mkdir -p "$dest_dir" 2>"$mkdir_err"; then
        echo "telemetry-rollup: failed to create destination directory: $(cat "$mkdir_err" 2>/dev/null)" >&2
        rm -f "$mkdir_err"
        return 0
    fi
    rm -f "$mkdir_err"

    local write_err
    write_err=$(mktemp)
    if ! printf '%s\n' "$line" >>"$destination" 2>"$write_err"; then
        echo "telemetry-rollup: failed to append to destination: $(cat "$write_err" 2>/dev/null)" >&2
        rm -f "$write_err"
        return 0
    fi
    rm -f "$write_err"

    return 0
}

main "$@" || true
exit 0
