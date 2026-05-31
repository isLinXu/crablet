---
title: Contributing
description: How to contribute code, report bugs, and propose features
---

# :handshake: Contributing

Thank you for your interest in contributing to Crablet! This guide will help you get started.

## Development Environment

**Prerequisites:**

- Rust 1.80+ (via [rustup](https://rustup.rs))
- Docker (optional, for sandbox and Neo4j)
- sccache (recommended, speeds up compilation)

```bash
# Clone and build
git clone https://github.com/isLinXu/crablet.git
cd crablet

# Minimal build (recommended for daily dev)
cargo build --no-default-features --features web

# Full build
cargo build --release

# Run tests
cargo test --release

# Lint
cargo clippy -- -D warnings

# Format check
cargo fmt --check
```

## Contribution Areas

### Most Welcome Contributions

| Area | Path | Difficulty |
|:-----|:-----|:----------:|
| New tool plugins | `src/tools/` | ![Beginner](https://img.shields.io/badge/Beginner-green) |
| New channel adapters | `src/channels/` | ![Intermediate](https://img.shields.io/badge/Intermediate-yellow) |
| Cognitive middleware | `src/cognitive/middleware.rs` | ![Intermediate](https://img.shields.io/badge/Intermediate-yellow) |
| Skill packages | `skills/` | ![Beginner](https://img.shields.io/badge/Beginner-green) |
| LLM adapters | `src/cognitive/llm/` | ![Intermediate](https://img.shields.io/badge/Intermediate-yellow) |
| Documentation | `docs/` | ![Beginner](https://img.shields.io/badge/Beginner-green) |

## Reporting Bugs

Use [GitHub Issues](https://github.com/isLinXu/crablet/issues) with the Bug Report template. Include:

- Crablet version (`crablet --version`)
- OS and Rust version
- Minimal reproduction steps
- Expected vs actual behavior

## Submitting Pull Requests

1. Fork and create a branch: `git checkout -b feature/my-feature`
2. Write code and tests
3. Ensure all checks pass: `cargo test && cargo clippy && cargo fmt --check`
4. Submit PR with a clear description

## Commit Convention

```
feat: add weather tool with OpenMeteo API
fix: resolve memory leak in working memory consolidation
docs: update README with Docker deployment guide
refactor: extract middleware pipeline into separate module
test: add integration tests for ReAct engine
```

## Code Standards

- Follow Rust standard style (`cargo fmt`)
- Add doc comments to all public API
- New features must include unit or integration tests
- Pass `cargo clippy` without warnings
