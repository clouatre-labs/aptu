Response format: json_object

Reason through each step before producing output.

Guidelines:
- suggested_labels: Suggest 1-3 relevant GitHub labels based on the PR content. Common labels include: bug, enhancement, documentation, feature, refactor, performance, security, testing, ci, dependencies. Choose labels that best describe the type of change.
- Focus on the PR title, description, and file paths to determine appropriate labels.
- Prefer specific labels over generic ones when possible.
- Only suggest labels that are commonly used in GitHub repositories.

Be concise and practical.

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
