---
title: Three-Layer Architecture
description: Understanding Crablet's System 1/2/3 cognitive layers
---

# :brain: Three-Layer Architecture

Crablet implements a three-layer cognitive architecture inspired by Daniel Kahneman's dual-process theory, extended with a third meta-cognitive layer.

## System 1 — Fast Intuition

**Purpose**: Rapid, intuitive responses for simple queries and routine tasks.

- Direct LLM inference without extensive reasoning
- Optimized for latency (< 500ms typical)
- Used for greetings, simple Q&A, and known-pattern responses
- Low computational cost

```
You: Hello!

🦀 Crablet [System 1]: Hi! How can I help you today?
```

## System 2 — Slow Reasoning

**Purpose**: Deliberate, step-by-step reasoning for complex problems.

- ReAct (Reasoning + Acting) loop with tool calls
- Multi-step decomposition and planning
- Self-consistency checking
- Higher latency but more accurate results

```
You: Analyze the performance bottlenecks in this codebase

🦀 Crablet [System 2]:
  🔍 Step 1: Examining project structure...
  🔍 Step 2: Identifying hot paths...
  🔍 Step 3: Profiling memory allocations...
  🔍 Step 4: Checking async patterns...
  
  Analysis complete. Found 3 bottlenecks:
  1. Excessive clone() in distributed_harness.rs
  2. Blocking I/O in channel handlers
  3. Unbounded concurrency in swarm coordinator
```

## System 3 — Meta-Cognition

**Purpose**: Self-monitoring, reflection, and continuous improvement.

- Monitors execution quality and confidence scores
- Diagnoses problems when confidence drops
- Learns patterns from successes and failures
- Optimizes future strategy selection

See [Meta-Cognition](meta-cognition.md) for detailed documentation.

## Cognitive Router

The router decides which system to engage based on:

| Signal | Routes To |
|:-------|:----------|
| Simple greeting/small talk | System 1 |
| Known-pattern query | System 1 |
| Multi-step task | System 2 |
| Low-confidence result | System 3 |
| Explicit reasoning request | System 2 |
| Error or failure | System 3 |

## Implementation

```rust
// src/cognitive/routing/mod.rs
pub enum CognitiveLayer {
    System1,  // Fast intuition
    System2,  // Slow reasoning
    System3,  // Meta-cognition
}

pub struct CognitiveRouter {
    complexity_threshold: f64,
    confidence_threshold: f64,
    pattern_matcher: PatternMatcher,
}
```
