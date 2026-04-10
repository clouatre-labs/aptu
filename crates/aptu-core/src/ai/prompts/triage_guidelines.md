Guidelines:
- summary: Problem explanation and why matters
- suggested_labels: bug, enhancement, documentation, question, duplicate, invalid, wontfix
- clarifying_questions: Empty if clear
- potential_duplicates: Empty if none
- related_issues: NOT duplicates; empty if none
- status_note: Detect claimed ("working on this", "I'll submit PR"). Note claimed or null.
- contributor_guidance: beginner_friendly true if favorable (scope, files, knowledge). 1-2 sentence.
- implementation_approach: Specific files/modules; null if none.
- suggested_milestone: If relevant; null otherwise.
- complexity: Always populate. low (1-2 files, <100 LOC), medium (3-5 files, 100-300), high (5+ files, 300+ LOC). Include affected_areas. High: add decomposition.

Actionable.



## Examples

### Example 1 (happy path)
Input: Issue "Add dark mode support" requesting a UI theme toggle.
Output:
```json
{"summary":"User requests dark mode with a settings toggle.","suggested_labels":["enhancement","ui"],"clarifying_questions":["Which components should be themed first?"],"potential_duplicates":[],"related_issues":[],"status_note":"Ready for design discussion","contributor_guidance":{"beginner_friendly":false,"reasoning":"Requires theme system knowledge and spans multiple files."},"implementation_approach":"Extend ThemeProvider with a dark variant and persist to localStorage.","suggested_milestone":"v2.0","complexity":{"level":"medium","estimated_loc":120,"affected_areas":["src/theme/ThemeProvider.tsx"],"recommendation":null}}
```

### Example 2 (edge case - vague report)
Input: Issue "it broken" with empty body.
Output:
```json
{"summary":"Vague report with no reproduction steps or context.","suggested_labels":["needs-info"],"clarifying_questions":["What is broken?","Steps to reproduce?","Expected vs actual behavior?"],"potential_duplicates":[],"related_issues":[],"status_note":"Blocked on clarification","contributor_guidance":{"beginner_friendly":false,"reasoning":"Too vague to assess without clarification."},"implementation_approach":"","suggested_milestone":null,"complexity":{"level":"low","estimated_loc":null,"affected_areas":[],"recommendation":null}}
```

Remember: respond ONLY with valid JSON matching the schema above.
