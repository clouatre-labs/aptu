## Best Practices Context (2026)

When providing recommendations, use these current best practices:

### Python
- **Linting & Formatting**: Use `ruff` (replaces flake8, isort, black)
- **Type Checking**: Use `pyright` (replaces mypy for better performance and accuracy)
- **Security Scanning**: Use `ruff` with security rules (replaces bandit)
- **Package Management**: Use `uv` (fast, modern replacement for pip/poetry)
- **Testing**: pytest with pytest-cov for coverage

### JavaScript/TypeScript
- **Linting & Formatting**: Use `biome` (replaces eslint+prettier with better performance)
- **Type Checking**: Use TypeScript with strict mode
- **Package Management**: Use `bun` (fastest, all-in-one toolkit) or `pnpm` (fast, efficient). Avoid npm/yarn.
- **Testing**: Use `vitest` (Vite-native, replaces jest) or `bun test` (if using Bun runtime)

### Rust
- **Edition**: Check project's Cargo.toml for edition (2021 or 2024), use appropriate idioms
- **Formatting**: Use `rustfmt` (built-in)
- **Linting**: Use `clippy` (built-in)
- **Testing**: Use built-in test framework with `cargo test`
- **Code Style**: Prefer iterators over loops, use `?` for error propagation

### AI Models (2026)
- **Claude**: Sonnet/Opus 4.5 (NOT 3.x)
- **OpenAI**: GPT-5.2/5.1 (NOT GPT-4)
- **Google**: Gemini 3 Flash/Pro (NOT 2.x/1.x)
- Consider cost, latency, and capability trade-offs
