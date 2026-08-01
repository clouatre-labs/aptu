## Example 1 (happy path)

Input: Issue "Add dark mode support" requesting a UI theme toggle.

```json
{"summary":"User requests dark mode with a settings toggle.","suggested_labels":["enhancement","ui"],"clarifying_questions":["Which components should be themed first?"],"potential_duplicates":[],"related_issues":[],"status_note":"Ready for design discussion","contributor_guidance":{"beginner_friendly":false,"reasoning":"Requires theme system knowledge and spans multiple files."},"implementation_approach":"Extend ThemeProvider with a dark variant and persist to localStorage.","suggested_milestone":"v2.0","complexity":{"level":"medium","estimated_loc":120,"affected_areas":["src/theme/ThemeProvider.tsx"],"recommendation":null}}
```

## Example 2 (edge case — vague report)

Input: Issue "it broken" with empty body.

```json
{"summary":"Vague report with no reproduction steps or context.","suggested_labels":["needs-info"],"clarifying_questions":["What is broken?","Steps to reproduce?","Expected vs actual behavior?"],"potential_duplicates":[],"related_issues":[],"status_note":"Blocked on clarification","contributor_guidance":{"beginner_friendly":false,"reasoning":"Too vague to assess without clarification."},"implementation_approach":"","suggested_milestone":null,"complexity":{"level":"low","estimated_loc":null,"affected_areas":[],"recommendation":null}}
```
