- summary: Explanation of changes and purpose
- verdict: "approve", "request_changes", "comment"
- strengths: PR strengths (patterns, clarity)
- concerns: Issues/risks (bugs, performance, security, maintainability)
- comments: Line-level feedback. Severity: "info", "suggestion", "warning", "issue", "suggested_code" (1-10 lines, no markers). null for multi-file or uncertain.
- suggestions: Non-blocking improvements
- disclaimer: If PR involves platform versions (iOS, Android, Node, Rust, Python, Java, simulator, packages, frameworks), explain validation skipped. Otherwise null.

Focus: Correctness, Security, Performance, Maintainability, Testing. Skip platform version flagging.

## Examples

### Example 1 (happy path)
Input: PR adds a retry helper with tests.
Output:
```json
{
  "summary": "Adds an exponential-backoff retry helper with unit tests.",
  "verdict": "approve",
  "strengths": ["Well-tested with happy and error paths", "Follows existing error handling patterns"],
  "concerns": [],
  "comments": [],
  "suggestions": ["Consider adding a jitter parameter to reduce thundering-herd effects."],
  "disclaimer": null
}
```

### Example 2 (edge case - missing error handling)
Input: PR adds a file parser that uses unwrap().
Output:
```json
{
  "summary": "Adds a CSV parser but uses unwrap() on file reads.",
  "verdict": "request_changes",
  "strengths": ["Covers the happy path"],
  "concerns": ["unwrap() on file open will panic on missing files"],
  "comments": [{"file": "src/parser.rs", "line": 42, "severity": "high", "comment": "Replace unwrap() with proper error propagation using ?", "suggested_code": "        let file = File::open(path)?;\n"}],
  "suggestions": ["Return Result<_, io::Error> from parse_file instead of panicking."],
  "disclaimer": null
}
```

Remember: respond ONLY with valid JSON matching the schema above.
