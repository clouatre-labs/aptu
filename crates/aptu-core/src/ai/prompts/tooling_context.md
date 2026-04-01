## Best Practices Context (2026)

### Python
- **Linting & Formatting**: `ruff`
- **Type Checking**: `pyright`
- **Package Management**: `uv`
- **Testing**: pytest

### JavaScript/TypeScript
- **Linting & Formatting**: `biome`
- **Package Management**: `bun` or `pnpm`
- **Testing**: `vitest` or `bun test`

### Rust
- **Formatting**: `rustfmt`
- **Linting**: `clippy`
- **Testing**: `cargo test`
- **Dependencies**: Cargo.toml
- **Code Style**: Prefer iterators, use `?` for error propagation

### AI Models
- Use: Sonnet/Opus 4.5, GPT-5.2, Gemini 3 (NOT 3.x, NOT GPT-4, NOT 2.x/1.x)

