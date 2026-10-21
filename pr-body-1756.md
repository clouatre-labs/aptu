## Summary

On the final budget tier, `drop_patches_by_size` now truncates over-budget patches to their leading hunks (diff header plus line-boundary-snapped hunks) with the marker `[APTU: patch truncated due to prompt budget -- do not speculate on missing content]`, instead of dropping them wholesale. A non-empty PR can therefore degrade to a truncated-diff review rather than failing with `EmptyReviewContext`. Truncations are recorded in `budget_drops` and the affected filenames in `truncated_patch_files` telemetry.

## Related Issues

Closes #1756

## Changes

- `ai/review_context/budget.rs`: final-tier truncation in `drop_patches_by_size` — per-patch budget as an even split of the remaining budget (200-char floor, marker length reserved), `truncate_at_line_boundary` snapping, saturating size decrement, `patch_truncated:{filename}` telemetry entries. Non-final tiers keep whole-patch drops; the `EmptyReviewContext` guard stays as a backstop for truly empty PRs.
- `ai/prompts/mod.rs`: parses `patch_truncated:` entries and records filenames into `truncated_patch_files`, consistent with existing recording.
- `ai/review_context/mod.rs`: updated 3 tests that assumed whole-patch drops; added final-tier truncation and oversized-patch tests; the all-patches-evicted regression test now asserts a truncated-patch `Ok` instead of `EmptyReviewContext`.

## Test Plan

- Happy paths: patches exceeding the budget produce truncated patches with the marker and reduced size, not `EmptyReviewContext`; a non-empty PR over budget yields a truncated-patch review context.
- Edge cases: a single patch larger than the whole budget truncates to header plus hunk (never empty); non-final tiers still drop whole patches; empty-PR backstop unchanged.
- Gates: `cargo nextest run --workspace --locked --no-fail-fast` (718 passed), `cargo nextest run -p aptu-core --locked --features ast-context` (639 passed), `cargo clippy -- -W clippy::cognitive_complexity`, `cargo fmt --check`, `cargo deny check advisories licenses`. Local wasm32 check hit the known E0463 toolchain quirk (no new wasm-gated code added); CI `WASM Check` is authoritative.

## Checklist

- [x] Tests pass locally
- [x] Linter/formatter clean
- [x] Commits are signed and follow conventional commit format
- [x] DCO signed-off: `git commit --signoff`
- [x] No scope creep (changes match assigned issue)
- [x] No secrets, API keys, or credentials in diff
- [x] I have reviewed every line in this PR and can explain it
