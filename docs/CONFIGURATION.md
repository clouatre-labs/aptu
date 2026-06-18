# Configuration

Config file: `~/.config/aptu/config.toml`

```toml
[ai]
provider = "gemini"  # or "cerebras", "groq", "openrouter", "zai", "zenmux"
model = "gemini-3.1-flash-lite-preview"
allow_paid_models = true  # default: allows paid OpenRouter models

[ui]
confirm_before_post = true
```

## Task-Specific Model Configuration

Configure different AI models for different operations (triage, review, create) to optimize for speed, cost, or reasoning depth:

```toml
[ai]
provider = "openrouter"
model = "mistralai/mistral-small-2603"  # default model for all tasks

# Override models for specific tasks
[ai.tasks.triage]
model = "mistralai/mistral-small-2603"  # fast and cheap for triage

[ai.tasks.review]
provider = "openrouter"
model = "anthropic/claude-haiku-4.5"  # balanced for review

[ai.tasks.create]
model = "anthropic/claude-sonnet-4.6"  # more capable for code creation
```

All task-specific overrides are optional. If not specified, the default `provider` and `model` are used.

### Task Configuration Options

- **`[ai.tasks.triage]`**: Configuration for issue triage operations
  - `provider`: Optional provider override
  - `model`: Optional model override

- **`[ai.tasks.review]`**: Configuration for code review operations
  - `provider`: Optional provider override
  - `model`: Optional model override

- **`[ai.tasks.create]`**: Configuration for code creation operations
  - `provider`: Optional provider override
  - `model`: Optional model override

## AI Provider Fallback Chain

Configure a fallback chain to automatically try alternative providers when the primary provider fails with a non-retryable error:

```toml
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

# Fallback chain: try these providers in order if primary fails
[ai.fallback]
chain = ["cerebras", "groq"]
```

Each fallback entry can optionally override the model for that specific provider:

```toml
[ai]
provider = "gemini"
model = "gemini-3.1-flash-lite-preview"

[ai.fallback]
chain = [
  { provider = "cerebras", model = "qwen-3-32b" },
  { provider = "groq", model = "llama-3.3-70b-versatile" }
]
```

When the primary provider fails with a non-retryable error (after retry exhaustion), Aptu will automatically try each provider in the fallback chain. If a fallback entry specifies a model override, that model is used; otherwise, the primary model is used. Rate limit and circuit breaker errors are not retried via fallback.

**Use Cases:**
- Resilience against provider outages
- Automatic failover for quota exhaustion
- Multi-provider redundancy for critical workflows
- Per-provider model optimization

**Notes:**
- Fallback only triggers for non-retryable errors
- Each fallback provider must have a valid API key configured
- Model overrides are optional; if not specified, the primary model is used
- Fallback attempts are logged with `warn` level tracing

## CLI Overrides

Override the configured provider and model with global flags:

```bash
aptu --provider openrouter --model mistralai/mistral-small-2603 issue triage owner/repo#123
```

Flags can be used independently (`--model` alone uses configured provider). CLI flags take precedence over config file.

## AI Provider Setup

Model IDs and pricing change frequently. Use `aptu models list` to discover available models from any configured provider.

Aptu supports multiple AI providers. Choose the one that works best for you:

### Anthropic

1. Get an API key from [Anthropic Console](https://console.anthropic.com/keys)
2. Set the environment variable:
   ```bash
   export ANTHROPIC_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "anthropic"
   model = "claude-haiku-4-5"
   ```

**Claude OAuth:** Aptu also reads `~/.claude/credentials.json` (written by the Claude desktop app or `claude` CLI) as an alternative to setting `ANTHROPIC_API_KEY`. If that file exists and contains a valid OAuth token, it is used automatically; no config change is needed.

**Prompt Caching:** Anthropic models support prompt caching via cache control tokens on system messages. Aptu automatically enables caching for all Anthropic requests; no additional configuration is required.

### Cerebras

1. Get an API key from [Cerebras Console](https://console.cerebras.ai/keys)
2. Set the environment variable:
   ```bash
   export CEREBRAS_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "cerebras"
   model = "qwen-3-32b"
   ```

**Free Tier:** Available with Cerebras API account

### Google AI Studio (Gemini)

1. Get a free API key from [Google AI Studio](https://aistudio.google.com/apikey)
2. Set the environment variable:
   ```bash
   export GEMINI_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "gemini"
   model = "gemini-3.1-flash-lite-preview"
   ```

Use `aptu models list --provider gemini` to discover current model IDs.

**Free Tier:** 15 requests/minute, 1M+ tokens/day, 1M token context window

### Groq

1. Get an API key from [Groq Console](https://console.groq.com/keys)
2. Set the environment variable:
   ```bash
   export GROQ_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "groq"
   model = "llama-3.3-70b-versatile"
   ```

**Free Tier:** Generous rate limits, fast inference with Groq's LPU technology

### OpenRouter

1. Get an API key from [OpenRouter](https://openrouter.ai/keys)
2. Set the environment variable:
   ```bash
   export OPENROUTER_API_KEY="sk-or-..."
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "openrouter"
   model = "mistralai/mistral-small-2603"
   ```

**Free Models:** Look for models with `:free` suffix on OpenRouter

### Z.AI (Zhipu)

1. Get an API key from [Z.AI](https://z.ai)
2. Set the environment variable:
   ```bash
   export ZAI_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "zai"
   model = "glm-4.5-air"
   ```

**Budget Tier:** glm-4.5-air with 128K context window (pricing subject to change; see Z.AI documentation)

### ZenMux

1. Get an API key from [ZenMux](https://zenmux.ai)
2. Set the environment variable:
   ```bash
   export ZENMUX_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "zenmux"
   model = "x-ai/grok-code-fast-1"
   ```

**Free Tier:** x-ai/grok-code-fast-1 with 256K context window

## PR Review Limits

Control how much context `aptu pr review` fetches and injects into the AI prompt:

```toml
[review]
max_prompt_chars = 120000          # Total prompt character budget (default: 120 000)
max_full_content_files = 10        # Max files fetched in full via GitHub Contents API (default: 10)
max_chars_per_file = 16000         # Max chars of full file content per file (default: 16 000)
max_diff_chars = 200000            # Max total diff characters across all files in the prompt (default: 200 000) — added in 0.10
max_patch_chars_per_file = 10000   # Max chars per individual file patch; patches exceeding this are dropped entirely (default: 10 000) — added in 0.10
max_instructions_chars = 1500      # Max chars of instructions file content included in review prompt (default: 1 500)
min_budget_for_call_graph = 20000  # Prompt chars remaining threshold below which call graph enrichment is skipped; set to 0 to always include call graph when repo-path is available (default: 20 000)
max_dep_packages = 3               # Max dependency bump packages for which upstream release notes are fetched (default: 3)
max_dep_release_chars = 2000       # Max chars of upstream release notes included per dependency package (default: 2 000)
```

The call graph is enabled only when `budget_remaining > min_budget_for_call_graph`, where
`budget_remaining = max_prompt_chars - estimated_size` (estimated size excludes the call graph itself).
Setting `min_budget_for_call_graph >= max_prompt_chars` disables call graph enrichment entirely.
Setting it above half of `max_prompt_chars` means call graph will only be built for the largest diffs.
The prefix section "When the assembled prompt exceeds..." describes how call graph is the first section dropped,
so a value that rarely enables call graph is typically acceptable.

When the assembled prompt exceeds `max_prompt_chars`, sections are dropped in this order: call-graph context, AST context, full file content (largest files first), diff hunks (largest first). The system prompt and PR metadata are never dropped.

## Cache Configuration

Control caching behavior for issues, repositories, and file-based cache entries:

```toml
[cache]
issue_ttl_minutes = 60      # TTL for cached issue responses (default: 60)
repo_ttl_hours = 24         # TTL for cached repository metadata (default: 24)
file_eviction_days = 7      # Age threshold for evicting file-based cache entries (default: 7)
```

- **`issue_ttl_minutes`**: How long (in minutes) to cache AI responses for issue triage before refetching. Set to 0 to disable caching.
- **`repo_ttl_hours`**: How long (in hours) to cache repository metadata (stars, description, topics, etc.) before refetching. Set to 0 to disable caching.
- **`file_eviction_days`**: Age threshold (in days) for removing stale cache files from disk. Must be greater than 0. Older cache files are automatically cleaned up.

All three keys are optional. Omitting any key restores the default listed above.

## Prompt Injection Protection

### Input Size Limits

Aptu enforces per-field byte limits before inserting user-supplied content into AI prompts. This bounds indirect prompt-injection surface (OWASP LLM01:2025).

```toml
[prompt]
max_issue_body_bytes    = 32768   # 32 KiB  — issue body
max_diff_bytes          = 524288  # 512 KiB — PR diff (injection-defence pre-check)
max_commit_message_bytes = 4096   # 4 KiB   — individual commit messages
```

When a field exceeds its limit:

- **CLI** (`aptu issue triage`, `aptu pr review`): exits non-zero with a diagnostic message.
- **MCP server**: returns a `ToolExecutionError` so the model can self-correct.

Note: `aptu scan-security` performs local pattern matching and does not invoke AI; these limits do not apply to it.

## DCO Sign-off (`dco_signoff`)

When creating a branch and commit via `aptu pr create --diff`, Aptu can append a
`Signed-off-by` trailer to the commit message to satisfy
[Developer Certificate of Origin](https://developercertificate.org/) requirements.

### Global default (config.toml)

Set `dco_signoff = true` in the `[repos]` section of `~/.config/aptu/config.toml` to
enable sign-off for every repository:

```toml
[repos]
dco_signoff = true
```

### Per-repository override (repos.toml)

Override the global default for a specific repository in `~/.config/aptu/repos.toml`:

```toml
[[repo]]
owner = "clouatre-labs"
name = "aptu"
dco_signoff = true   # require DCO for this repo regardless of global default

[[repo]]
owner = "some-org"
name = "permissive-project"
dco_signoff = false  # opt out even if global default is true
```

The per-repo value always takes precedence over the global default. The `--dco-signoff`
CLI flag on `aptu pr create` overrides both.

## Prompt Customization

Aptu ships with built-in system prompts compiled into the binary. You can override them at runtime without rebuilding.

### Append custom guidance (all operations)

Add a `custom_guidance` field to `~/.config/aptu/config.toml`. The text is appended to every system prompt, after the built-in tooling context:

```toml
[ai]
provider = "openrouter"
model = "mistralai/mistral-small-2603"
custom_guidance = "Always respond in French. Prefer concise labels."
```

Use this for project-wide conventions you want the AI to follow consistently across triage, review, and create operations.

### Replace a system prompt for a specific operation

Drop a Markdown file at `~/.config/aptu/prompts/<operation>.md`. If the file exists and is readable, it fully replaces the built-in system prompt for that operation. The `custom_guidance` field is still appended on top.

Supported operation names:

| File | Replaces |
|------|----------|
| `~/.config/aptu/prompts/triage.md` | Issue triage system prompt |
| `~/.config/aptu/prompts/review.md` | PR review system prompt |
| `~/.config/aptu/prompts/pr_label.md` | PR label suggestion system prompt |
| `~/.config/aptu/prompts/create.md` | Issue creation system prompt |

**Example:** customize the triage prompt for a monorepo:

```bash
mkdir -p ~/.config/aptu/prompts
cp $(cargo locate-project --workspace --message-format plain | xargs dirname)/crates/aptu-core/src/ai/prompts/triage_guidelines.md \
   ~/.config/aptu/prompts/triage.md
# Now edit ~/.config/aptu/prompts/triage.md with your changes
```

### Developer note

Built-in prompt fragments live in `crates/aptu-core/src/ai/prompts/` (guidelines as `.md`, response schemas as `.json`) and are embedded at compile time via `include_str!`. The builder functions (`build_triage_system_prompt`, etc.) in `prompts/mod.rs` are shared between production code and `tests/prompt_lint.rs` to guarantee tests exercise real construction logic.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `APTU_CONTEXT_FILE` | Path to write a JSONL file containing per-review context records for explainability and debugging. Each line is a JSON object with fields: `pr_url`, `repo`, `total_chars`, `budget_drops` (list of enrichment steps skipped due to budget), and `prompt_chars_final`. If unset, no file is written. |
| `APTU_METRICS_FILE` | Path to write a JSONL file containing per-review token usage metrics. Used by the GitHub Action to capture `aptu-token-usage.jsonl` as an artifact. |
