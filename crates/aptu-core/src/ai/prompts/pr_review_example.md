## Example 1 (happy path)

Input: PR adds a retry helper with tests.

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

## Example 2 (edge case — missing error handling)

Input: PR adds a file parser that uses unwrap().

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
