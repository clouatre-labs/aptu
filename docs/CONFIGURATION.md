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

Aptu supports multiple AI providers. Choose the one that works best for you:

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

**Free Tier:** 15 requests/minute, 1M+ tokens/day, 1M token context window

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

**Budget Tier:** glm-4.5-air with 128K context window ($0.20/$1.10 per 1M tokens)

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
max_prompt_chars = 120000      # Total prompt character budget (default: 120 000)
max_full_content_files = 10    # Max files fetched in full via GitHub Contents API (default: 10)
max_chars_per_file = 4000      # Max characters per full-content file snippet (default: 4 000)
```

When the assembled prompt exceeds `max_prompt_chars`, sections are dropped in this order: call-graph context, AST context, full file content (largest files first), diff hunks (largest first). The system prompt and PR metadata are never dropped.

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
| `~/.config/aptu/prompts/release_notes.md` | Release notes system prompt |

**Example:** customize the triage prompt for a monorepo:

```bash
mkdir -p ~/.config/aptu/prompts
cp $(cargo locate-project --workspace --message-format plain | xargs dirname)/crates/aptu-core/src/ai/prompts/triage_guidelines.md \
   ~/.config/aptu/prompts/triage.md
# Now edit ~/.config/aptu/prompts/triage.md with your changes
```

### Developer note

Built-in prompt fragments live in `crates/aptu-core/src/ai/prompts/` (guidelines as `.md`, response schemas as `.json`) and are embedded at compile time via `include_str!`. The builder functions (`build_triage_system_prompt`, etc.) in `prompts/mod.rs` are shared between production code and `tests/prompt_lint.rs` to guarantee tests exercise real construction logic.
