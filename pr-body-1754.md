## Summary

Classifies provider context-length errors (HTTP 400/413 whose body mentions context/token length) as a typed `AptuError::ContextLengthExceeded`, and recovers at the facade level with exactly one input re-trim: the review closure rebuilds the review context with a halved effective prompt budget so `apply_budget_drops` applies its semantic drop order, then retries the AI call once. A second context-length failure propagates. The re-trim is recorded in `budget_drops` telemetry.

## Related Issues

Closes #1754

## Changes

- `error.rs`: new `AptuError::ContextLengthExceeded { message }` variant.
- `ai/provider/http.rs`: `map_http_error` classifies 400/413 with body markers (`context_length_exceeded`, `prompt is too long`, `reduce the length`, `maximum context`); other 400s stay generic. Provider layer stays PR-agnostic.
- `retry.rs`: the variant is deliberately non-retryable in `is_retryable_anyhow` (see Deviation below).
- `facade/ai_client.rs`: `try_with_fallback` propagates the typed error instead of entering the fallback chain (an oversized prompt fails on every provider).
- `facade/pr_review/analyze.rs`: `retry_once_on_context_length` rebuilds the context at half the effective budget and retries exactly once; pushes `context_length_retrim` into `budget_drops`.
- `aptu-cli/errors.rs`: actionable tip for the new variant (exhaustive match).

## Deviation from issue

The issue asks to classify the error as retryable in `is_retryable_anyhow`. Instead, the variant is non-retryable at the provider layer and the single bounded retry lives in the facade (`analyze.rs`). The provider retry loop holds only a serialized request and cannot re-trim; retrying there would resend an identical oversized prompt and double-retry against the facade. The observable outcome matches the issue's acceptance criteria: one re-trimmed retry, not a hard failure.

## Test Plan

- Happy paths: 413 and 400-with-length-body produce `ContextLengthExceeded`; one mocked context-length failure triggers exactly one rebuilt-context retry that succeeds.
- Edge cases: 400 without length markers stays generic `AptuError::AI`; `is_retryable_anyhow` returns false for the variant; a second context-length failure propagates.
- Gates: `cargo nextest run --workspace --locked --no-fail-fast` (714 passed), `cargo nextest run -p aptu-core --locked --features ast-context` (635 passed), `cargo clippy -- -W clippy::cognitive_complexity`, `cargo fmt --check`. The local wasm32 check fails on this machine for dependency crates identically on unmodified `main` (E0463, Homebrew rustc vs rustup shim environment quirk); the new code adds no OS-only paths, and CI's `WASM Check` is the authoritative gate.

## Checklist

- [x] Tests pass locally
- [x] Linter/formatter clean
- [x] Commits are signed and follow conventional commit format
- [x] DCO signed-off: `git commit --signoff`
- [x] No scope creep (changes match assigned issue)
- [x] No secrets, API keys, or credentials in diff
- [x] I have reviewed every line in this PR and can explain it
