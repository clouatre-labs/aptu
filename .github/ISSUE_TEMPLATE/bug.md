---
name: Bug
about: Report a defect or unexpected behavior
title: "[BUG] "
labels: bug
assignees: ""
---

## Summary
<!-- 1-2 sentences: what is broken? Be specific about the symptom, not the suspected cause. Include the command you ran. -->

Example: "When I run `aptu issue triage https://github.com/org/repo/issues/123`, the tool panics instead of returning a triage result."

## Steps to Reproduce
<!-- Numbered list of exact steps to trigger the bug. Include commands, flags, and input data. -->

1. Run `aptu auth login`
2. Run `aptu issue triage <url>`
3. Observe the unexpected behavior

## Expected Behavior
<!-- What should happen? -->

The command should complete successfully and return structured output.

## Actual Behavior
<!-- What actually happens? Include error messages, stack traces, or unexpected output. -->

```
<paste full error output or stack trace here>
```

## Environment
<!-- Provide context for reproduction. -->

- Aptu version: `aptu --version`
- OS: macOS / Linux / Windows (specify version)
- AI provider and model: e.g., OpenRouter / mistralai/devstral-2512:free
- Rust version (if building from source): `rustc --version`

## Logs / Error Output
<!-- Full error message, panic backtrace, or relevant log output. Use code block. Set RUST_LOG=debug for verbose output. -->

```
<paste full error output here>
```

## Root Cause Analysis
<!-- Optional: if you have a hypothesis about what's wrong, include it here. Reference file paths and line ranges. -->

Example: "Suspected cause: `src/triage/mod.rs` line 42 calls `.unwrap()` on a `Result` that can fail when the GitHub API returns a 404. Should use `?` operator with a typed error instead."

## Fix Direction
<!-- Optional: suggested approach or pattern to follow. Reference existing code patterns. -->

Example: "Replace `.unwrap()` with `?` operator. Use `thiserror` to define a custom error type. Follow the error handling pattern in `src/review/mod.rs` (lines 50-75)."

## Acceptance Criteria
<!-- Checkbox list of verifiable outcomes. -->

- [ ] Bug is reproducible with provided steps
- [ ] Root cause identified and documented
- [ ] Fix implemented without introducing new panics
- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt --check`
- [ ] Regression test added covering the bug scenario
- [ ] Commit GPG signed and DCO signed-off
