# Aptu - Project Specification

> **APTU** - AI-Powered Triage Utility
> 
> Also: (Mi'kmaq) "Paddle" - Navigate forward through open source contribution

**Domains:** [aptu.dev](https://aptu.dev) | [aptu.app](https://aptu.app)

## 1. Project Overview

### Vision
A gamified mobile app that helps developers contribute meaningfully to open source projects through AI-assisted issue triage and PR review, with skill-based progression from beginner-friendly repos to complex codebases.

### Goals
1. **Be useful** - Reduce maintainer burden, not add noise
2. **Learn by doing** - Build with Rust backend, SwiftUI frontend, GitHub API
3. **Quality over quantity** - Reward depth of contribution, not volume
4. **Progressive skill building** - Start simple, unlock complexity

### Learning Objectives
| Technology | Skills to Develop |
|------------|-------------------|
| Rust | Backend API, async I/O, GitHub client, JSON parsing |
| SwiftUI | Mobile UI, OAuth flow, offline storage, networking |
| GitHub API | REST/GraphQL, webhooks, OAuth apps, rate limiting |
| AI Integration | API calls, prompt engineering, output validation |

---

## 2. Problem Statement

### For Contributors
- **Overwhelming choice** - Where do I start? Which repos need help?
- **Fear of rejection** - Will my contribution be good enough?
- **No feedback loop** - Did my triage/review actually help?
- **Skill mismatch** - Complex codebases intimidate newcomers

### For Maintainers
- **Issue backlog** - Hundreds of untriaged issues
- **Review burden** - PRs sit for weeks without feedback
- **Low-quality contributions** - "+1" comments, trivial PRs, spam
- **Onboarding cost** - Time spent guiding new contributors

### Gap in Market
| Existing Tool | What It Does | What It Lacks |
|---------------|--------------|---------------|
| CodeTriage | Emails open issues daily | No gamification, no AI, no progression |
| PR Triage | Automates PR labeling | No skill progression, no community |
| Gemini CLI Actions | AI-powered automation | Not community-focused, no mobile |
| Gitcolony | Code review workflows | No gamification, no AI assistance |

**Aptu fills the gap:** Gamification + AI assistance + skill progression + mobile-first

---

## 3. Target Users

### Primary: New-to-Intermediate Contributors
- Want to contribute to OSS but don't know where to start
- Have 1-5 hours/week for contributions
- Motivated by learning, recognition, and impact
- Comfortable with code but not expert-level

### Secondary: OSS Maintainers
- Need help triaging issues and reviewing PRs
- Want quality contributions, not noise
- Willing to approve/reject triaged work
- Active projects with regular releases

### Anti-Targets (Not Serving)
- Expert contributors who already have workflows
- Inactive/abandoned repositories
- Spam contributors gaming metrics

---

## 4. MVP Scope (Phase 1)

### In Scope: Rust CLI for Issue Triage
Build a command-line tool that:
1. Authenticates with GitHub via OAuth (device flow)
2. Fetches "good first issue" from curated repos
3. Displays issue with AI-generated summary
4. Suggests labels and clarifying questions
5. Allows user to submit triage comment
6. Tracks local history of contributions

### Out of Scope for MVP
- iOS app (Phase 2)
- PR review (Phase 2+)
- Gamification (points, badges, leaderboards) (Phase 3)
- Multi-user features (Phase 3)
- Monetization (Phase 4)
- Web interface (Maybe never)

### Success Criteria for MVP
- [ ] CLI works end-to-end on a real repo
- [ ] AI summaries are useful (user validates)
- [ ] At least 1 maintainer accepts a triage from Aptu
- [ ] Author learns Rust fundamentals through building

---

## 5. Technical Architecture

### Phase 1: CLI Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Aptu CLI                           │
├─────────────────────────────────────────────────────────┤
│  Commands:                                              │
│  - aptu auth login/logout/status  (GitHub OAuth)        │
│  - aptu repo list                 (curated repos)       │
│  - aptu issue list [REPO]         (good-first-issues)   │
│  - aptu issue triage <URL>        (AI triage + comment) │
│  - aptu history                   (contribution log)    │
│  - aptu completion <SHELL>        (shell completions)   │
└─────────────────────────────────────────────────────────┘
           │                    │
           ▼                    ▼
┌──────────────────┐  ┌──────────────────┐
│   GitHub API     │  │    AI Provider   │
│  (REST/GraphQL)  │  │ (Mistral/Claude) │
└──────────────────┘  └──────────────────┘
```

### Command Structure

```
aptu
├── auth
│   ├── login              # Authenticate with GitHub
│   ├── logout             # Remove stored credentials
│   └── status             # Show current auth state
├── repo
│   └── list               # List curated repositories
├── issue
│   ├── list [REPO]        # List issues (optional positional)
│   └── triage <URL>       # Triage an issue with AI
├── history                 # Show contribution history
└── completion <SHELL>      # Generate shell completions
```

### Global CLI Flags

All commands support these flags for LLM-friendly and scripted usage:

| Flag | Short | Description |
|------|-------|-------------|
| `--output <format>` | `-o` | Output format: `text` (default), `json`, `yaml`, `markdown` |
| `--quiet` | `-q` | Suppress non-essential output (spinners, progress) |

**Command-specific flags:**
| Flag | Command | Description |
|------|---------|-------------|
| `--dry-run` | `issue triage` | Preview triage without posting |
| `--yes` / `-y` | `issue triage` | Skip confirmation prompt |

**Examples:**
```bash
# JSON output for LLM parsing
aptu issue list block/goose --output json

# Check auth status quietly
aptu auth status --quiet

# Auto-confirm triage for automation
aptu issue triage https://github.com/org/repo/issues/123 --yes

# Generate shell completions
aptu completion zsh > ~/.zsh/completions/_aptu
```

### Rust Crates

#### Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.x | CLI argument parsing (derive API) |
| `tokio` | 1.x | Async runtime (full features) |
| `reqwest` | 0.12.x | HTTP client for APIs |
| `serde` | 1.x | Serialization framework |
| `serde_json` | 1.x | JSON parsing and validation |

#### GitHub Integration

| Crate | Version | Purpose |
|-------|---------|---------|
| `octocrab` | latest | GitHub API client (OAuth, issues, comments) |
| `secrecy` | 0.10.x | Secure handling of tokens in memory |

#### Error Handling and Logging

| Crate | Version | Purpose |
|-------|---------|---------|
| `thiserror` | 2.x | Custom error type derivation |
| `anyhow` | 1.x | Application-level error handling with context |
| `tracing` | 0.1.x | Structured logging and diagnostics |
| `tracing-subscriber` | 0.3.x | Log output configuration (env filter, formatting) |

#### Configuration and Storage

| Crate | Version | Purpose |
|-------|---------|---------|
| `config` | 0.14.x | Layered configuration (file + env + defaults) |
| `keyring` | 3.x | Secure credential storage (system keychain) |
| `dirs` | 5.x | XDG-compliant config/data paths |

#### User Experience

| Crate | Version | Purpose |
|-------|---------|---------|
| `indicatif` | 0.17.x | Progress bars and spinners |
| `dialoguer` | 0.11.x | Interactive prompts and confirmations |
| `console` | 0.15.x | Terminal styling and colors |

#### Testing

| Crate | Version | Purpose |
|-------|---------|---------|
| `tokio-test` | 0.4.x | Async test utilities |
| `wiremock` | 0.6.x | HTTP mocking for API tests |
| `assert_cmd` | 2.x | CLI integration testing |

### Data Storage (Local)

```
~/.config/aptu/
├── config.toml          # User configuration
└── repos.toml           # Curated repository list (optional override)

~/.local/share/aptu/
├── history.json         # Contribution history
└── cache/               # Cached API responses
    ├── issues/          # Issue data (TTL: 1 hour)
    └── repos/           # Repository metadata (TTL: 24 hours)
```

- **Config:** `~/.config/aptu/config.toml` (layered with env vars)
- **Auth token:** System keychain via `keyring` (never plaintext)
- **History:** `~/.local/share/aptu/history.json`
- **Cache:** `~/.local/share/aptu/cache/` (with TTL expiration)

### 5.1 Error Handling Strategy

**Pattern:** `thiserror` for library errors, `anyhow` for application errors.

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AptuError {
    #[error("GitHub API error: {0}")]
    GitHub(#[from] octocrab::Error),

    #[error("AI provider error: {message}")]
    AI { message: String, status: Option<u16> },

    #[error("Authentication required - run `aptu auth` first")]
    NotAuthenticated,

    #[error("Rate limit exceeded, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Invalid JSON response from AI")]
    InvalidAIResponse(#[source] serde_json::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

// In main.rs - use anyhow for top-level error handling
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = load_config()
        .context("Failed to load configuration")?;
    
    // ... application logic
    Ok(())
}
```

**Error Display:** Show user-friendly messages, log full details at debug level.

### 5.2 Logging Strategy

**Pattern:** Structured logging with `tracing`, environment-based filtering.

```rust
use tracing::{info, debug, error, warn, instrument};
use tracing_subscriber::EnvFilter;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("aptu=info".parse().unwrap())
                .add_directive("octocrab=warn".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap())
        )
        .with_target(false)  // Cleaner CLI output
        .init();
}

// Usage throughout codebase
#[instrument(skip(client), fields(repo = %repo, issue = issue_num))]
async fn fetch_issue(client: &Octocrab, repo: &str, issue_num: u64) -> Result<Issue> {
    debug!("Fetching issue details");
    let issue = client.issues(owner, repo).get(issue_num).await?;
    info!(labels = ?issue.labels, "Issue fetched successfully");
    Ok(issue)
}
```

**Log Levels:**
- `error!` - Unrecoverable failures
- `warn!` - Recoverable issues (rate limits, retries)
- `info!` - Key operations (auth, triage submitted)
- `debug!` - Detailed flow (API calls, cache hits)
- `trace!` - Verbose debugging (request/response bodies)

**Environment Variable:** `RUST_LOG=aptu=debug` for troubleshooting.

### 5.3 Configuration Schema

**File:** `~/.config/aptu/config.toml`

```toml
# User preferences
[user]
default_repo = "block/goose"  # Optional: skip repo selection
maintainer_mode = false       # Triage your own repos privately (Phase 2+)

# AI provider settings
[ai]
provider = "openrouter"  # openrouter (Phase 1) | ollama (Phase 2+)
model = "mistralai/devstral-2512:free"
allow_paid_models = false  # Safety: require explicit opt-in for paid models
timeout_seconds = 30

# GitHub settings
[github]
api_timeout_seconds = 10

# UI preferences
[ui]
color = true
progress_bars = true
confirm_before_post = true  # Always ask before posting comments

# Cache settings
[cache]
issue_ttl_minutes = 60
repo_ttl_hours = 24
```

**Environment Variable Overrides:**
- `APTU_AI_PROVIDER` → `ai.provider`
- `APTU_AI_MODEL` → `ai.model`
- `OPENROUTER_API_KEY` → AI API key (stored in keychain after first use)
- `GITHUB_TOKEN` → Override OAuth token (for CI/testing)

---

## 6. Feature Requirements

### 6.1 Authentication (`aptu auth`)

#### Subcommands

| Command | Description |
|---------|-------------|
| `aptu auth login` | Authenticate with GitHub via OAuth device flow |
| `aptu auth logout` | Remove stored credentials from keychain |
| `aptu auth status` | Show current authentication state and token source |

#### Authentication Priority Chain

Aptu checks for GitHub credentials in this order:

1. **Environment variable** - `GH_TOKEN` or `GITHUB_TOKEN` (for CI/scripting)
2. **GitHub CLI** - `gh auth token` if `gh` is installed (piggyback on existing auth)
3. **Native OAuth** - `aptu auth login` device flow (primary interactive method)

This means users with `gh` CLI already installed don't need to authenticate again.

#### OAuth App Registration (Developer Setup)

Aptu uses a registered **OAuth App** with GitHub. The Client ID and Client Secret are
hardcoded in the source code (safe for public/native clients per OAuth 2.0 spec).

**Why OAuth App instead of GitHub App?**
- OAuth Apps are simpler for user-facing CLI tools
- GitHub Apps are better for bots/automation acting independently
- `gh` CLI uses the same pattern (OAuth App with embedded credentials)

**One-time setup (maintainer only):**
1. Go to <https://github.com/settings/developers>
2. Click "New OAuth App"
3. Fill in:
   - Application name: `Aptu CLI`
   - Homepage URL: `https://github.com/clouatre-labs/project-aptu`
   - Authorization callback URL: `http://127.0.0.1/callback` (not used for device flow)
4. Save Client ID and Client Secret in source code constants
5. Enable Device Flow: Settings > Developer settings > OAuth Apps > [Your App] > Enable Device Flow
   (One-time setup; required for CLI/headless clients; without this, device flow requests return HTTP 400)

**Reference:** See `gh` CLI source: [internal/authflow/flow.go](https://github.com/cli/cli/blob/trunk/internal/authflow/flow.go)

#### Native OAuth Device Flow (`aptu auth login`)

When no existing token is found:

1. User runs `aptu auth login`
2. CLI requests device code from GitHub using embedded Client ID
3. CLI displays: "Visit https://github.com/login/device and enter code: XXXX-XXXX"
4. User authorizes in browser
5. CLI polls for access token
6. Token stored in system keychain via `keyring` crate

```rust
// Example using octocrab device flow
use octocrab::Octocrab;
use secrecy::SecretString;

const APTU_CLIENT_ID: &str = "abc123...";  // Hardcoded after OAuth App registration

let client_id = SecretString::from(APTU_CLIENT_ID);
let codes = crab.authenticate_as_device(&client_id, ["repo", "read:org"]).await?;
println!("Go to {} and enter code {}", codes.verification_uri, codes.user_code);
let auth = codes.poll_until_available(&crab, &client_id).await?;
```

#### Required OAuth Scopes

| Scope | Purpose |
|-------|---------|
| `repo` | Read issues, create comments, access private repos |
| `read:org` | List organization memberships (for org-owned repos) |
| `gist` | Optional: for future gist-based features |

#### Token Storage

- **Primary:** System keychain via `keyring` crate (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- **Fallback:** `~/.config/aptu/token` with `600` permissions (if keychain unavailable)
- **Never:** Environment variables for persistent storage (only for CI override)

### 6.2 Repository Discovery (`aptu repo list`)

**MVP:** Hardcoded list of 10-20 curated repos known to be:
- Active (commits in last 30 days)
- Welcoming ("good first issue" labels exist)
- Responsive (maintainers reply within 1 week)

**Future:** Dynamic discovery via GitHub search API

**Output Example:**
```
Available repositories:

  1. block/goose          Rust     42 open issues   Last active: 2 days ago
  2. astral-sh/ruff       Python   128 open issues  Last active: 1 day ago
  3. tauri-apps/tauri     Rust     89 open issues   Last active: 3 days ago
```

### 6.3 Issue Listing (`aptu issue list [REPO]`)

**Implementation:** Uses GitHub GraphQL API for efficient label filtering.

**Current Filters (MVP):**
- Label: `good first issue` (exact match, case-insensitive)
- State: Open
- Limit: 20 issues per request

**Future Filters:**
- Additional labels: "help wanted"
- No assignee filter
- Date range filter (e.g., created in last 90 days)

**Output Example:**
```
Issues in block/goose:

  #1234  [bug]         "CLI crashes on empty config"     3 days ago
  #1189  [enhancement] "Add --verbose flag"              1 week ago
  #1156  [docs]        "README missing install steps"    2 weeks ago
```

### 6.4 Issue Triage (`aptu issue triage <URL>`)

**Workflow:**
1. Fetch issue details (title, body, comments, labels)
2. Fetch related context (linked PRs, similar issues)
3. Call AI for analysis:
   - Summary (2-3 sentences)
   - Suggested labels
   - Clarifying questions for reporter
   - Potential duplicates
4. Display to user for review/edit
5. User confirms -> post comment to GitHub

**AI Prompt Structure:**

*System Prompt:*
```
You are an OSS issue triage assistant. Analyze the provided GitHub issue and provide structured triage information.

Your response MUST be valid JSON with this exact schema:
{
  "summary": "A 2-3 sentence summary of what the issue is about and its impact",
  "suggested_labels": ["label1", "label2"],
  "clarifying_questions": ["question1", "question2"],
  "potential_duplicates": ["#123", "#456"]
}

Guidelines:
- summary: Concise explanation of the problem/request and why it matters
- suggested_labels: Choose from: bug, enhancement, documentation, question, good first issue, help wanted, duplicate, invalid, wontfix
- clarifying_questions: Only include if the issue lacks critical information. Leave empty array if issue is clear.
- potential_duplicates: Only include if you detect likely duplicates from the context. Leave empty array if none.

Be helpful, concise, and actionable. Focus on what a maintainer needs to know.
```

*User Prompt:*
```
<issue_content>
Title: {title}

Body:
{body}

Existing Labels: {labels}

Recent Comments:
- @{author}: {comment_body}
</issue_content>
```

**Context Engineering Parameters:**

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `temperature` | 0.3 | Low temperature for deterministic, consistent triage output |
| `max_tokens` | 1024 | Sufficient for structured JSON response |
| `response_format` | `json_object` | OpenRouter structured output mode (model-dependent) |
| Body truncation | 4000 chars | Stay within token limits |
| Max comments | 5 | Limit context size |
| Comment truncation | 500 chars each | Prevent single comment from dominating |

**Security: Prompt Injection Mitigation**
- Delimit user content with `<issue_content>...</issue_content>` tags
- Truncate inputs to prevent token overflow attacks
- Validate JSON schema of AI response via `serde_json`
- Never execute code from AI output
- Show AI output to user before posting (interactive confirmation)

### 6.5 Contribution History (`aptu history`)

**Local tracking:**
```json
{
  "contributions": [
    {
      "id": "uuid",
      "repo": "block/goose",
      "issue": 1234,
      "action": "triage",
      "timestamp": "2024-12-13T23:00:00Z",
      "comment_url": "https://github.com/...",
      "status": "pending"  // pending | accepted | rejected
    }
  ]
}
```

**Future:** Sync with server for cross-device, leaderboards

---

## 7. API Design

### GitHub API Endpoints

**REST API:**

| Action | Endpoint | Method |
|--------|----------|--------|
| Device auth start | `/login/device/code` | POST |
| Device auth poll | `/login/oauth/access_token` | POST |
| Get user | `/user` | GET |
| List issues | `/repos/{owner}/{repo}/issues` | GET |
| Get issue | `/repos/{owner}/{repo}/issues/{number}` | GET |
| Search issues | `/search/issues` | GET |
| Create comment | `/repos/{owner}/{repo}/issues/{number}/comments` | POST |

**GraphQL API:**

| Action | Endpoint | Notes |
|--------|----------|-------|
| List issues with labels | `https://api.github.com/graphql` | Used by `aptu issues` for efficient label filtering |

GraphQL is preferred for issue listing because it allows filtering by label in a single request, avoiding the need to fetch all issues and filter client-side.

### Rate Limits
- Authenticated: 5,000 requests/hour
- Search API: 30 requests/minute
- **Mitigation:** Cache repo/issue data locally, use conditional requests (ETags)

### 7.1 HTTP Client Configuration

**Timeouts:**
```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(30))         // Total request timeout
    .connect_timeout(Duration::from_secs(5))  // Connection timeout
    .build()?;
```

**Retry Strategy:**
- Retry on: 429 (rate limit), 500, 502, 503, 504
- Max retries: 3
- Backoff: Exponential with jitter (1s, 2s, 4s + random 0-500ms)
- Check `Retry-After` header for rate limits

```rust
async fn with_retry<T, F, Fut>(operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempts = 0;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < 3 && is_retryable(&e) => {
                let delay = Duration::from_millis(
                    (1000 * 2u64.pow(attempts)) + rand::random::<u64>() % 500
                );
                warn!(attempt = attempts + 1, delay_ms = ?delay.as_millis(), "Retrying after error");
                tokio::time::sleep(delay).await;
                attempts += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### AI Provider API

**Primary Provider:** OpenRouter (unified API)
- Endpoint: `https://openrouter.ai/api/v1/chat/completions`
- Default Model: `mistralai/devstral-2512:free` (FREE, 262K context)
- Fallback Model: `mistralai/mistral-small-3.1-24b-instruct:free` (FREE, 128K context)

**OpenRouter Benefits:**
- Single API for multiple providers (Mistral, Anthropic, xAI)
- Free tier models available
- Easy model switching for premium tiers
- OpenAI-compatible API format

**Alternative:** Direct Mistral API
- Endpoint: `https://api.mistral.ai/v1/chat/completions`
- Model: `devstral-small-2505` or `mistral-small-latest`

**Future: Local Ollama Support (Phase 2+)**
- Endpoint: `http://localhost:11434/api/chat`
- Model: `mistral:7b` or `codellama:7b`

**Future Premium Models (via OpenRouter):**
- `x-ai/grok-4.1-fast` - 2M context, $0.20/$0.50 per 1M tokens
- `anthropic/claude-sonnet-4.5` - 1M context, $3/$15 per 1M tokens

### 7.2 AI Provider Authentication

Unlike GitHub (which supports OAuth device flow), AI providers require manual API key setup.

**Current Approach:** Manual API key creation and copy-paste.

1. CLI directs user to `https://openrouter.ai/settings/keys`
2. User creates key and copies it
3. User provides key via:
   - Interactive prompt (stored in system keychain)
   - Environment variable `OPENROUTER_API_KEY`

**Why Not OAuth/Device Flow?**

We investigated OAuth device flow for AI providers to match the GitHub auth experience.
None currently support it. We've submitted feature requests:

| Provider | Feature Request | Status |
|----------|-----------------|--------|
| OpenRouter | [openrouter-examples#61](https://github.com/OpenRouterTeam/openrouter-examples/issues/61) | Open |
| Hugging Face | [huggingface_hub#3628](https://github.com/huggingface/huggingface_hub/issues/3628) | Open |
| Mistral | [client-python#295](https://github.com/mistralai/client-python/issues/295) | Open |

**Alternative Considered:** Localhost HTTPS callback server (like `gh auth login --web`).
Rejected for MVP due to complexity (self-signed certs, browser warnings, port conflicts).

**Future:** If providers implement device flow, we'll adopt it for consistent UX.

---

## 8. Security Considerations

### Authentication
- **OAuth credentials in source:** Client ID and Client Secret are embedded in source code
  (safe for native/CLI apps per [OAuth 2.0 for Native Apps RFC 8252](https://tools.ietf.org/html/rfc8252))
- **Token storage:** System keychain via `keyring` crate (never plaintext files)
- **Token refresh:** OAuth tokens from device flow are long-lived; refresh before expiry
- **Logout:** `aptu auth logout` revokes token via GitHub API and removes from keychain
- **Fallback chain:** Check env vars and `gh` CLI before prompting for new auth

### Prompt Injection
- Treat all issue content as untrusted
- Use structured prompts with clear delimiters
- Validate AI output against JSON schema
- Never auto-post without user confirmation
- Log AI interactions for debugging

### AI Output Disclaimer
- LLM outputs are suggestions only
- Final responsibility for triage quality lies with the contributor
- Always review AI-generated content before posting

### Rate Limiting
- Respect GitHub's rate limits (check headers)
- Implement exponential backoff
- Cache aggressively (issues don't change often)
- Warn user when approaching limits

### Privacy
- No telemetry without consent
- Local-first data storage
- No PII sent to AI except issue content (already public)

---

## 9. Monetization Strategy (Future)

### AI Provider Strategy

**Primary Provider:** OpenRouter (unified API for multiple models)

#### Free Tier Models (via OpenRouter)

| Model | Context | Cost | Best For |
|-------|---------|------|----------|
| `mistralai/devstral-2512:free` | 262K | FREE | Code-focused triage (default) |
| `mistralai/mistral-small-3.1-24b-instruct:free` | 128K | FREE | General triage (fallback) |

#### Paid Tier Models

| Model | Context | Cost (per 1M tokens) | Best For |
|-------|---------|----------------------|----------|
| `x-ai/grok-code-fast-1` | 256K | $0.20 / $1.50 | Code review (cheap) |
| `x-ai/grok-4.1-fast` | 2M | $0.20 / $0.50 | Large context (cheap) |
| `anthropic/claude-sonnet-4.5` | 1M | $3 / $15 | Highest quality |
| `anthropic/claude-haiku-4.5` | 200K | $1 / $5 | Good balance |

#### Cost per Triage (avg 1,500 input + 500 output tokens)

| Model | Cost per Triage | 100 Triages/month |
|-------|-----------------|-------------------|
| Devstral 2 (free) | $0.00 | $0.00 |
| Grok Code Fast 1 | ~$0.001 | ~$0.10 |
| Claude Sonnet 4.5 | ~$0.012 | ~$1.20 |

### Freemium Model

| Tier | Price | AI Model | Features |
|------|-------|----------|----------|
| Free | $0 | Devstral 2 / Mistral Small 3.1 | 50 triages/day, basic features |
| Plus | $4.99/mo | Grok Code Fast 1 / Grok 4.1 Fast | Unlimited, faster, 2M context |
| Pro | $14.99/mo | Claude Sonnet 4.5 | PR review, team features, priority API |

### Additional Revenue Streams
- **Badges/Cosmetics** - Custom profile flair (in-app purchase)
- **Enterprise** - Team dashboards, private repos, SSO
- **Sponsorships** - Partner with OSS foundations for branded challenges
- **GitHub Sponsors** - Community-funded development

### Cost Structure (Estimate)
- AI API: $0.00 per triage (free tier), ~$0.001-0.012 (paid tiers)
- GitHub API: Free (within rate limits)
- Hosting: Minimal for CLI (user's machine)

---

## 10. Success Metrics

### Phase 1 (CLI MVP)
| Metric | Target |
|--------|--------|
| CLI builds and runs | Yes |
| Successful OAuth flow | Yes |
| AI summaries rated "useful" by author | 80%+ |
| Triage comments accepted by maintainers | 1+ |
| Rust learning objectives met | Self-assessed |

### Phase 2 (iOS App)
| Metric | Target |
|--------|--------|
| App Store launch | Yes |
| Daily active users | 100+ |
| Triages submitted | 500+ |
| Maintainer approval rate | 70%+ |

### Phase 3 (Gamification)
| Metric | Target |
|--------|--------|
| User retention (30-day) | 40%+ |
| Points redeemed | Tracked |
| Premium conversions | 5%+ of active users |

---

## 11. Roadmap

### Phase 1: Rust CLI (Weeks 1-4)
- [x] Project setup (Cargo, CI, README)
- [x] GitHub OAuth device flow
- [x] Fetch issues from hardcoded repos
- [x] AI integration (OpenRouter API with Devstral 2)
- [x] Triage command with user confirmation
- [x] Local history tracking
- [ ] Test with 1-2 real repos

### Phase 2: iOS App (Weeks 5-10)
- [ ] SwiftUI project setup
- [ ] Rust -> Swift bridge (FFI or REST)
- [ ] OAuth flow in iOS
- [ ] Issue browser UI
- [ ] Triage submission UI
- [ ] TestFlight beta

### Phase 3: Gamification (Weeks 11-16)
- [ ] Points system design
- [ ] Backend for user profiles
- [ ] Leaderboards (global, per-repo, weekly/monthly/all-time)
- [ ] Badges and achievements
- [ ] Skill progression (unlock complex repos)

#### Leaderboard Design (Phase 3)

**Leaderboard Types:**
- **Global** - All-time top contributors across all repos
- **Per-Repo** - Top contributors to specific repositories
- **Time-based** - Weekly, monthly, and all-time rankings
- **Skill-tier** - Separate boards for beginner/intermediate/advanced

**Ranking Criteria:**
- Quality-weighted triages (accepted > pending > rejected)
- Maintainer approval rate
- Consistency (streak bonuses)
- Difficulty multiplier (complex repos = more points)

**Anti-Gaming Measures:**
- Minimum quality threshold to appear on leaderboard
- Rate limiting (max triages per day that count)
- Maintainer rejection penalty
- Manual review for suspicious patterns

### Phase 4: Monetization (Weeks 17+)
- [ ] Premium AI tier integration
- [ ] In-app purchases (iOS)
- [ ] Subscription management
- [ ] Analytics and conversion tracking

---

## 12. Open Questions

1. **Repo curation:** How do we identify "good" repos? Manual curation vs. automated scoring?
2. **Maintainer opt-in:** Should repos explicitly opt-in to Aptu contributions?
3. **Quality scoring:** How do we measure if a triage was "good" beyond maintainer approval?
4. **Offline mode:** Cache issues for offline viewing on mobile?

---

## 13. References

- [Mistral Conversation](https://chat.mistral.ai/chat/b8fe356e-725e-4a57-b5fb-1204c91fded3) - Initial brainstorm
- [Grok Conversation](https://grok.com/share/c2hhcmQtMw_71925307-d251-4afe-aae1-73c1bc76181a) - Market analysis
- [GitHub OAuth Device Flow](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow) - Device flow documentation
- [GitHub REST API](https://docs.github.com/en/rest) - API documentation
- [GitHub Apps vs OAuth Apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/differences-between-github-apps-and-oauth-apps) - When to use which
- [OAuth 2.0 for Native Apps (RFC 8252)](https://tools.ietf.org/html/rfc8252) - Why client secrets in source are OK
- [gh CLI authflow source](https://github.com/cli/cli/blob/trunk/internal/authflow/flow.go) - Reference implementation
- [cli/oauth Go library](https://github.com/cli/oauth) - OAuth device/web flow library used by gh
- [OpenRouter](https://openrouter.ai/) - Unified AI API provider
- [Octocrab](https://github.com/xampprocky/octocrab) - Rust GitHub API client
- [CodeTriage](https://www.codetriage.com/) - Competitor reference
- [Mistral API](https://docs.mistral.ai/) - AI provider docs
- [cc-sdd/OpenSpec](https://github.com/cc-sdd/OpenSpec) - Specification format inspiration

---

*Last updated: 2025-12-15*
*Author: Hugues Clouatre*
