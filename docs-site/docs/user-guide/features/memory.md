---
title: Memory
description: Persistent four-layer memory across sessions
---

# :floppy_disk: Memory

Crablet implements a four-layer memory architecture that persists across sessions, enabling the agent to learn and improve over time.

For detailed documentation, see the [Memory System](../memory/index.md) section.

## Quick Facts

| Property | Value |
|:---------|:------|
| Memory layers | 4 (Working, Episodic, Semantic, Procedural) |
| Persistence | SQLite + optional Neo4j/Qdrant |
| Consolidation | Automatic (time + quantity triggered) |
| Retrieval | Fusion (multi-layer hybrid) |

## Memory Management

```bash
# View memory statistics
crablet status --memory

# Clear working memory
crablet memory clear --layer working

# Export episodic memories
crablet memory export --layer episodic --format json
```
