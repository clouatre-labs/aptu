---
name: Feature
about: Propose a new feature or enhancement
title: "[FEATURE] "
labels: enhancement
assignees: ""
---

## Summary
<!-- 1-2 sentences describing what to build. Be specific: what capability is missing, what user need does it address? -->

Example: "Add a `--filter` flag to `aptu issue list` that allows filtering by label, assignee, or state."

## Context
<!-- Why does this matter? What depends on it? Link to parent issues, roadmap, or design docs. Help agents understand the broader system impact. -->

- What problem does this solve?
- Which users or workflows benefit?
- Links to related issues, discussions, or spec sections (e.g., SPEC.md)
- Any blocking dependencies?

## Prerequisites
<!-- List any issues that must be completed first. Use "Depends on: #N" format. -->

- Depends on: #N (if applicable)

## Implementation Notes
<!-- The meat of the issue. Agents parse this to understand exactly what to build. Include code examples, API references, verified crate versions, design decisions, and integration points. -->

### Strategy
<!-- Numbered approaches or key decisions. Reference AGENTS.md and architecture docs where relevant. -->

1. **First approach or component**
   - Where to add: `src/module/submodule.rs`
   - Verified version: check `Cargo.lock` for installed version
   - API pattern: show expected usage
   - Integration: where and how to register or wire up

2. **Second approach or component**
   - Reference existing implementation: `src/module/existing.rs` (lines N-M for pattern)
   - Key considerations

### Code Examples
<!-- Show expected patterns, API usage, and integration points. -->

```rust
// Expected pattern
let result = some_function(args)?;
```

### Integration Notes
- Error handling: use `thiserror` for custom errors
- Logging: use `tracing` macros, not `println!`
- Testing: add tests covering happy path and edge cases
- CLI: follow `clap` patterns in existing subcommands

### API References
- SPEC.md: relevant section reference
- Upstream crate docs: link to docs.rs
- Related code: file paths and line ranges for patterns

## Acceptance Criteria
<!-- Checkbox list of verifiable outcomes. Agents use this to validate completion. -->

- [ ] Feature implemented per specification
- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt --check`
- [ ] Integration tests cover happy path and edge cases
- [ ] No `unwrap()`, `expect()`, or `println!` in library code
- [ ] Documentation updated (if applicable)
- [ ] Commit GPG signed and DCO signed-off

## Not In Scope
<!-- Explicit boundaries to prevent scope creep. -->

- Features or phases planned for later
- Performance optimization (unless specified above)
- Changes to public API or interfaces beyond what's specified
