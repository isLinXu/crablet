---
title: Memory System
description: Crablet's four-layer memory architecture
icon: material/database
---

# :database: Memory System

Crablet implements a four-layer memory architecture inspired by cognitive psychology, providing persistent context across sessions.

<div class="grid cards" markdown>

-   :brain: **Four-Layer Memory**
    
    Working → Episodic → Semantic → Procedural
    
    ---
    
    [:octicons-arrow-right-24: Four-Layer Details](four-layer.md)

-   :twisted_rightwards_arrows: **Memory Fusion**
    
    Intelligent retrieval combining all memory layers
    
    ---
    
    [:octicons-arrow-right-24: Fusion Details](fusion.md)

</div>

## Architecture Overview

```mermaid
graph TB
    Input[Sensory Input] --> WM[Working Memory<br/>Current Context]
    WM --> EM[Episodic Memory<br/>Experience Records]
    EM --> SM[Semantic Memory<br/>General Knowledge]
    SM --> PM[Procedural Memory<br/>Skills & Procedures]
    
    WM -.->|Consolidation| EM
    EM -.->|Abstraction| SM
    SM -.->|Practice| PM
    
    PM -.->|Retrieval| SM
    SM -.->|Recall| EM
    EM -.->|Association| WM
```

## Quick Facts

| Property | Value |
|:---------|:------|
| Memory layers | 4 (Working, Episodic, Semantic, Procedural) |
| Persistence | SQLite + optional Neo4j/Qdrant |
| Consolidation | Automatic (time + quantity triggered) |
| Retrieval | Fusion (multi-layer hybrid) |
| Thread safety | Arc&lt;RwLock&gt; throughout |
