---
name: Documentation
about: Improve documentation or add new docs
title: "[DOCS] "
labels: documentation
assignees: ""
---

## Summary
<!-- 1-2 sentences: what documentation to create or improve and why. Be specific about the gap. -->

Example: "The README doesn't explain how to configure the AI provider. Users need clear setup instructions for supported providers."

## Location
<!-- Where does this documentation go? File path, section in existing doc, or new file? -->

Example:
- Update section "Configuration" in `README.md`
- Or: Add to existing `SPEC.md` (section reference)
- Or: New file: `docs/providers.md`

## Current State
<!-- What exists today? What is missing or incomplete? -->

Current documentation (if any):
```
<quote from existing docs>
```

Gap: <specific missing information>

Example:
Current: "Run `aptu auth login` to authenticate."
Gap: No explanation of which AI providers are supported, how to select one, or how to configure API keys.

## Proposed Changes
<!-- What to add or update? Include the new content structure and key topics to cover. -->

### Content to Add/Update
- Section/heading: <what should be documented>
- Key topics: list of important points to cover
- Target audience: developers, users, contributors
- Format: narrative prose, API reference, tutorial, FAQ

### Structure
```
## Section Title

### Subsection
<explanation and examples>
```

## Acceptance Criteria
<!-- Checkbox list of verifiable outcomes. -->

- [ ] Documentation is clear and complete
- [ ] Examples are accurate and tested (if applicable)
- [ ] Target audience can follow instructions without external reference
- [ ] Formatting is consistent with existing docs
- [ ] Links and references are correct
- [ ] No outdated or broken references
- [ ] Commit GPG signed and DCO signed-off
