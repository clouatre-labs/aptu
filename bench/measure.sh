#!/bin/bash
# Measures system prompt sizes before/after prompt compression (#1096).
# Persona strings are extracted directly from crates/aptu-core/src/ai/prompts/mod.rs
# so this script always reflects the actual production prompts without duplication.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MOD_RS="$REPO_ROOT/crates/aptu-core/src/ai/prompts/mod.rs"

TOOLING_PATH="$REPO_ROOT/crates/aptu-core/src/ai/prompts/tooling_context.md"
TRIAGE_GUIDELINES_PATH="$REPO_ROOT/crates/aptu-core/src/ai/prompts/triage_guidelines.md"
PR_REVIEW_GUIDELINES_PATH="$REPO_ROOT/crates/aptu-core/src/ai/prompts/pr_review_guidelines.md"
CREATE_GUIDELINES_PATH="$REPO_ROOT/crates/aptu-core/src/ai/prompts/create_guidelines.md"
RELEASE_GUIDELINES_PATH="$REPO_ROOT/crates/aptu-core/src/ai/prompts/release_notes_guidelines.md"
PR_LABEL_GUIDELINES_PATH="$REPO_ROOT/crates/aptu-core/src/ai/prompts/pr_label_guidelines.md"

# Extract persona byte counts from mod.rs.
# Each build_*_system_prompt formats: "<persona>\n\n{context}\n\n{GUIDELINES}".
# The persona is everything in the format string before {context}.
# Rust line-continuation (\<newline><leading-spaces>) collapses to nothing;
# \n escape sequences become real newlines.
extract_persona_bytes() {
  python3 - "$MOD_RS" << 'PYEOF'
import re, sys

def extract_persona_bytes(src, fn_name):
    fn_idx = src.find(f'pub fn {fn_name}(')
    if fn_idx == -1:
        sys.exit(f"ERROR: function {fn_name} not found in mod.rs")
    fmt_start = src.find('format!(', fn_idx)
    str_start = src.find('"', fmt_start) + 1
    context_idx = src.find('{context}', str_start)
    persona_raw = src[str_start:context_idx]
    # Collapse Rust line-continuation: \ + newline + leading whitespace
    persona = re.sub(r'\\\n[ \t]*', '', persona_raw)
    # Interpret \n as real newline (only \n, not other escapes)
    persona = persona.replace('\\n', '\n')
    return len(persona.encode('utf-8'))

src = open(sys.argv[1]).read()
funcs = [
    ('triage',    'build_triage_system_prompt'),
    ('pr_review', 'build_pr_review_system_prompt'),
    ('create',    'build_create_system_prompt'),
    ('release',   'build_release_notes_system_prompt'),
    ('pr_label',  'build_pr_label_system_prompt'),
]
for key, fn in funcs:
    print(f"{key}={extract_persona_bytes(src, fn)}")
PYEOF
}

# Load persona byte counts as shell variables (PERSONA_triage, PERSONA_pr_review, ...)
while IFS='=' read -r key val; do
  eval "PERSONA_${key}=${val}"
done < <(extract_persona_bytes)

# Guidelines and tooling byte counts
TOOLING=$(wc -c < "$TOOLING_PATH")
TRIAGE_GUIDELINES=$(wc -c < "$TRIAGE_GUIDELINES_PATH")
PR_REVIEW_GUIDELINES=$(wc -c < "$PR_REVIEW_GUIDELINES_PATH")
CREATE_GUIDELINES=$(wc -c < "$CREATE_GUIDELINES_PATH")
RELEASE_GUIDELINES=$(wc -c < "$RELEASE_GUIDELINES_PATH")
PR_LABEL_GUIDELINES=$(wc -c < "$PR_LABEL_GUIDELINES_PATH")

# Totals (persona + tooling + guidelines)
TRIAGE_TOTAL=$((PERSONA_triage + TOOLING + TRIAGE_GUIDELINES))
PR_REVIEW_TOTAL=$((PERSONA_pr_review + TOOLING + PR_REVIEW_GUIDELINES))
CREATE_TOTAL=$((PERSONA_create + TOOLING + CREATE_GUIDELINES))
RELEASE_TOTAL=$((PERSONA_release + TOOLING + RELEASE_GUIDELINES))
PR_LABEL_TOTAL=$((PERSONA_pr_label + TOOLING + PR_LABEL_GUIDELINES))

# Print markdown table
cat <<EOF
| Operation | Persona (bytes) | Tooling (bytes) | Guidelines (bytes) | Total (bytes) |
|-----------|----------------|----------------|--------------------|---------------|
| triage    | $PERSONA_triage | $TOOLING | $TRIAGE_GUIDELINES | $TRIAGE_TOTAL |
| pr_review | $PERSONA_pr_review | $TOOLING | $PR_REVIEW_GUIDELINES | $PR_REVIEW_TOTAL |
| create    | $PERSONA_create | $TOOLING | $CREATE_GUIDELINES | $CREATE_TOTAL |
| release   | $PERSONA_release | $TOOLING | $RELEASE_GUIDELINES | $RELEASE_TOTAL |
| pr_label  | $PERSONA_pr_label | $TOOLING | $PR_LABEL_GUIDELINES | $PR_LABEL_TOTAL |
EOF

# Write sizes.json using jq for safe JSON construction
jq -n \
  --argjson triage_persona       "$PERSONA_triage" \
  --argjson tooling              "$TOOLING" \
  --argjson triage_guidelines    "$TRIAGE_GUIDELINES" \
  --argjson triage_total         "$TRIAGE_TOTAL" \
  --argjson pr_review_persona    "$PERSONA_pr_review" \
  --argjson pr_review_guidelines "$PR_REVIEW_GUIDELINES" \
  --argjson pr_review_total      "$PR_REVIEW_TOTAL" \
  --argjson create_persona       "$PERSONA_create" \
  --argjson create_guidelines    "$CREATE_GUIDELINES" \
  --argjson create_total         "$CREATE_TOTAL" \
  --argjson release_persona      "$PERSONA_release" \
  --argjson release_guidelines   "$RELEASE_GUIDELINES" \
  --argjson release_total        "$RELEASE_TOTAL" \
  --argjson pr_label_persona     "$PERSONA_pr_label" \
  --argjson pr_label_guidelines  "$PR_LABEL_GUIDELINES" \
  --argjson pr_label_total       "$PR_LABEL_TOTAL" \
  --arg     date                 "$(date -u +%Y-%m-%d)" \
  '{
    generated_at: $date,
    note: "Baseline measured pre-#1096; after values are null placeholders (pending #1096 merge).",
    operations: {
      triage:    { before: { persona_chars: $triage_persona,    tooling_chars: $tooling, guidelines_chars: $triage_guidelines,    total_chars: $triage_total    }, after: null, reduction_pct: null },
      pr_review: { before: { persona_chars: $pr_review_persona, tooling_chars: $tooling, guidelines_chars: $pr_review_guidelines, total_chars: $pr_review_total }, after: null, reduction_pct: null },
      create:    { before: { persona_chars: $create_persona,    tooling_chars: $tooling, guidelines_chars: $create_guidelines,    total_chars: $create_total    }, after: null, reduction_pct: null },
      release:   { before: { persona_chars: $release_persona,   tooling_chars: $tooling, guidelines_chars: $release_guidelines,   total_chars: $release_total   }, after: null, reduction_pct: null },
      pr_label:  { before: { persona_chars: $pr_label_persona,  tooling_chars: $tooling, guidelines_chars: $pr_label_guidelines,  total_chars: $pr_label_total  }, after: null, reduction_pct: null }
    }
  }' > "$REPO_ROOT/bench/results/sizes.json"
