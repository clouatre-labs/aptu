Guidelines:
- suggested_labels: 1-3 labels (bug, enhancement, documentation, feature, refactor, performance, security, testing, ci, dependencies). Use PR title, description, files.
- Prefer specific over generic. Use common labels.

Concise.



## Examples

### Example 1 (happy path)
Input: PR adds OAuth2 login flow with tests.
Output:
```json
{"suggested_labels": ["feature", "auth", "security"]}
```

### Example 2 (edge case - documentation only PR)
Input: PR fixes typos in README.
Output:
```json
{"suggested_labels": ["documentation"]}
```

Remember: respond ONLY with valid JSON matching the schema above.
