Create a curated release notes document with:
1. A theme/title that captures the essence of this release
2. A 1-2 sentence narrative about the release
3. 3-5 highlighted features
4. Categorized changes: Features, Fixes, Improvements, Documentation, Maintenance
5. List of contributors

Reason through each step before producing output.

Guidelines:
- theme: A short title (3-6 words) capturing the release essence. No version number.
- narrative: 1-2 sentences summarizing what changed and why it matters to users.
- highlights: 3-5 most impactful changes. Bold the feature name with a dash separator, include PR number in parentheses.
- features/fixes/improvements/documentation/maintenance: Categorize all PRs. Group by user impact, not commit type. Filter CI/deps changes under maintenance.
- contributors: List all PR authors prefixed with @.

Conventions:
- No emojis
- Bold feature names with dash separator (e.g., "**Dark mode** - adds theme toggle (#42)")
- Include PR numbers in parentheses
- Group by user impact, not just commit type

## Examples

### Example 1 (happy path)
Input: PRs adding retry logic, fixing a crash, updating docs.
Output:
```json
{
  "theme": "Reliability and Polish",
  "narrative": "This release focuses on reliability improvements and bug fixes that make the CLI more resilient in high-traffic environments.",
  "highlights": ["**Exponential backoff retry** - reduces failures under rate limits (#12)", "**Login crash fix** - resolves panic on invalid token (#15)"],
  "features": ["**Exponential backoff retry** - reduces failures under rate limits (#12)"],
  "fixes": ["**Login crash fix** - resolves panic on invalid token (#15)"],
  "improvements": [],
  "documentation": ["**README update** - adds quickstart guide (#16)"],
  "maintenance": [],
  "contributors": ["@alice", "@bob"]
}
```

### Example 2 (edge case - single contributor, maintenance-heavy)
Input: Mostly dependency bumps with one feature PR.
Output:
```json
{
  "theme": "Dependency Refresh",
  "narrative": "Routine dependency updates with one new export command.",
  "highlights": ["**Export command** - adds JSON export for triage results (#22)"],
  "features": ["**Export command** - adds JSON export for triage results (#22)"],
  "fixes": [],
  "improvements": [],
  "documentation": [],
  "maintenance": ["Bump tokio to 1.40 (#23)", "Bump serde to 1.0.215 (#24)"],
  "contributors": ["@carol"]
}
```

Remember: respond ONLY with valid JSON matching the schema above.