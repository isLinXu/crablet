---
title: FAQ
description: Frequently Asked Questions
---

# :question: FAQ

## General

### What is Crablet?

Crablet is a production-ready AI Agent Operating System built from scratch in Rust. It provides a complete framework for building, deploying, and managing AI agents with a three-layer cognitive architecture, four-layer memory system, and multi-channel support.

### Why is it called Crablet?

Because good AI infrastructure should be as reliable as a crab's shell, and as fast as it scuttles sideways. 🦀

### How does Crablet compare to other agent frameworks?

| Feature | Crablet | OpenClaw | Agent Zero |
|:--------|:--------|:---------|:-----------|
| Language | Rust | TypeScript | Python |
| Performance | ★★★★★ | ★★★ | ★★ |
| Memory System | 4-layer | 2-layer | Basic |
| Cognitive Architecture | 3-layer | None | None |
| Concurrency | 100+ agents | ~10 | ~5 |
| Binary Size | < 20 MB | ~100 MB | ~50 MB |

## Technical

### Which LLM providers are supported?

- **OpenAI** (GPT-4o, GPT-4o-mini, etc.)
- **DashScope** (Qwen series)
- **Ollama** (Local models)
- **Any OpenAI-compatible endpoint**

### Can I use Crablet without Docker?

Yes! Crablet runs natively on Linux, macOS, and Windows. Docker is only needed for the sandbox feature.

### How do I reduce memory usage?

```bash
# Build with minimal features
cargo build --release --no-default-features --features web

# Reduce working memory capacity in config
[memory.working]
capacity = 30  # Default is 50-100
```

### Is Crablet production-ready?

Crablet has comprehensive test coverage (347+ test functions, 40+ integration test files) and CI/CD pipelines. It's suitable for production deployment with proper configuration.

## Troubleshooting

### Build fails with "linker not found"

Install a C compiler and linker:

```bash
# Ubuntu/Debian
sudo apt install build-essential

# macOS
xcode-select --install
```

### WebSocket connection refused

Ensure the gateway is running:

```bash
crablet gateway --port 18789
# Or
crablet serve-web --port 18790
```

### LLM returns errors

Check your API key configuration:

```bash
echo $OPENAI_API_KEY
# Should output: sk-xxx...
```

### High memory usage

Reduce vector index sizes and working memory capacity:

```toml
[memory.working]
capacity = 30

[knowledge]
chunk_size = 256  # Smaller chunks = less memory
top_k = 5         # Fewer retrieved results
```
