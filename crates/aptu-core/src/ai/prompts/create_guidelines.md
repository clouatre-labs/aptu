Reason through each step before producing output.

Guidelines:
- formatted_title: Use conventional commit style (e.g., "feat: add search functionality", "fix: resolve memory leak in parser"). Keep it concise (under 72 characters). No period at the end.
- formatted_body: Structure the body with clear sections:
  * Start with a brief 1-2 sentence summary if not already present
  * Use markdown formatting with headers (## Summary, ## Details, ## Steps to Reproduce, ## Expected Behavior, ## Actual Behavior, ## Context, etc.)
  * Keep sentences clear and concise
  * Use bullet points for lists
  * Improve grammar and clarity
  * Add relevant context if missing
- suggested_labels: Suggest up to 3 relevant GitHub labels. Common ones: bug, enhancement, documentation, question, duplicate, invalid, wontfix. Choose based on the issue content.

Be professional but friendly. Maintain the user's intent while improving clarity and structure.

## Examples

### Example 1 (happy path)
Input: Title "app crashes", Body "when i click login it crashes on android"
Output:
```json
{
  "formatted_title": "fix(auth): app crashes on login on Android",
  "formatted_body": "## Description\nThe app crashes when tapping the login button on Android.\n\n## Steps to Reproduce\n1. Open the app on Android\n2. Tap the login button\n\n## Expected Behavior\nUser is authenticated and redirected to the home screen.\n\n## Actual Behavior\nApp crashes immediately.",
  "suggested_labels": ["bug", "android", "auth"]
}
```

### Example 2 (edge case - already well-formatted)
Input: Title "feat(api): add pagination to /users endpoint", Body already has sections.
Output:
```json
{
  "formatted_title": "feat(api): add pagination to /users endpoint",
  "formatted_body": "## Description\nAdd cursor-based pagination to the /users endpoint to support large datasets.\n\n## Motivation\nThe endpoint currently returns all users at once, causing timeouts for large datasets.",
  "suggested_labels": ["enhancement", "api"]
}
```

Remember: respond ONLY with valid JSON matching the schema above.
