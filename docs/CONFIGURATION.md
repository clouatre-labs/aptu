# Configuration

Config file: `~/.config/aptu/config.toml`

```toml
[ai]
provider = "gemini"  # or "openrouter", "groq", "cerebras"
model = "gemini-3-flash-preview"
allow_paid_models = false  # default: blocks paid OpenRouter models

[ui]
confirm_before_post = true
```

## AI Provider Setup

Aptu supports multiple AI providers. Choose the one that works best for you:

### Google AI Studio (Gemini) - Default

1. Get a free API key from [Google AI Studio](https://aistudio.google.com/apikey)
2. Set the environment variable:
   ```bash
   export GEMINI_API_KEY="your-api-key-here"
   ```
3. Configure in `~/.config/aptu/config.toml`:
   ```toml
   [ai]
   provider = "gemini"
   model = "gemini-3-flash-preview"
   ```

**Free Tier:** 15 requests/minute, 1M+ tokens/day, 1M token context window

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
   model = "mistralai/devstral-2512:free"
   ```

**Free Models:** Look for models with `:free` suffix on OpenRouter

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
