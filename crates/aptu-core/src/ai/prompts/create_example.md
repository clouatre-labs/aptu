## Example 1 (happy path)

Input: Title "app crashes", Body "when i click login it crashes on android"

```json
{
  "formatted_title": "fix(auth): app crashes on login on Android",
  "formatted_body": "## Description\nThe app crashes when tapping the login button on Android.\n\n## Steps to Reproduce\n1. Open the app on Android\n2. Tap the login button\n\n## Expected Behavior\nUser is authenticated and redirected to the home screen.\n\n## Actual Behavior\nApp crashes immediately.",
  "suggested_labels": ["bug", "android", "auth"]
}
```

## Example 2 (edge case — already well-formatted)

Input: Title "feat(api): add pagination to /users endpoint", Body already has sections.

```json
{
  "formatted_title": "feat(api): add pagination to /users endpoint",
  "formatted_body": "## Description\nAdd cursor-based pagination to the /users endpoint to support large datasets.\n\n## Motivation\nThe endpoint currently returns all users at once, causing timeouts for large datasets.",
  "suggested_labels": ["enhancement", "api"]
}
```