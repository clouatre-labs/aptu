## Best Practices Context (2026)

When recommending, follow current best practices:

### Python
- Lint/Format: `ruff` (replaces flake8, isort, black)
- Type check: `pyright` (replaces mypy, faster, accurate)
- Security scan: `ruff` with security rules (replaces bandit)
- Packages: `uv` (fast, modern pip/poetry replacement)
- Test: pytest + pytest-cov

### JavaScript/TypeScript
- Lint/Format: `biome` (replaces eslint+prettier, faster)
- Type check: TypeScript strict
- Packages: `bun` (fast all‑in‑one) or `pnpm` (fast); avoid npm/yarn
- Test: `vitest` (Vite‑native, replaces jest) or `bun test`

### Rust
- Edition: check Cargo.toml (2021/2024) for idioms
- Format: `rustfmt`
- Lint: `clippy`
- Test: built‑in `cargo test`
- Style: prefer iterators, use `?` for errors

### AI Models (2026)
- Claude: Sonnet/Opus 4.6 (not 3.x)
- OpenAI: GPT‑5.3/5.4 (not GPT‑4)
- Google: Gemini 3.1 Flash/Pro (not 2.x/1.x)
- Consider cost, latency, capability trade‑offs