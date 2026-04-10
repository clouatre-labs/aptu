Create curated release notes:
1. Theme
2. Narrative (1-2 sentences)
3. 3-5 impactful changes
4. Categories: Features, Fixes, Improvements, Docs, Maintenance
5. Contributors

Theme: 3-6 words. Narrative: Why it matters. "**Feature** - desc (#N)". By impact, CI/deps → Maintenance. No emojis.






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
