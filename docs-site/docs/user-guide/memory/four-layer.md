---
title: Four-Layer Memory
description: Working, Episodic, Semantic, and Procedural memory layers
---

# :brain: Four-Layer Memory

Crablet's memory system mirrors human cognition with four distinct layers, each serving a different purpose.

## Layer 1: Working Memory

**Purpose**: Hold the current conversation context and active thoughts.

- Stores recent messages, active goals, and current reasoning state
- Limited capacity (configurable, typically 50-100 items)
- Fastest access latency
- Automatically prunes oldest items when capacity reached

```rust
pub struct WorkingMemory {
    items: VecDeque<MemoryItem>,
    capacity: usize,
    decay_rate: f64,
}
```

## Layer 2: Episodic Memory

**Purpose**: Record specific experiences and events.

- Stores timestamped episodes with full context
- Enables "remember when..." recall
- Consolidated from working memory periodically
- Indexed by time, participants, and emotional valence

```rust
pub struct EpisodicMemory {
    episodes: Vec<Episode>,
    consolidation_threshold: usize,
    emotional_weight: f64,
}
```

## Layer 3: Semantic Memory

**Purpose**: Store generalized knowledge and facts.

- Abstracted from episodic memories over time
- Facts, concepts, relationships
- Powered by Neo4j for graph traversal
- Supports entity extraction and relationship mapping

```rust
pub struct SemanticMemory {
    graph: Neo4jClient,
    entities: HashMap<EntityId, Entity>,
    relations: Vec<Relation>,
}
```

## Layer 4: Procedural Memory

**Purpose**: Learned skills and procedures.

- Stores successful strategies and action sequences
- Enables "muscle memory" for repeated tasks
- Updated by meta-cognitive optimizer
- Maps situations to effective procedures

```rust
pub struct ProceduralMemory {
    procedures: HashMap<SituationType, Procedure>,
    effectiveness_scores: HashMap<ProcedureId, f64>,
}
```

## Consolidation Flow

Information flows upward through layers:

1. **Working → Episodic**: Periodic consolidation (every N turns or N minutes)
2. **Episodic → Semantic**: Abstraction over repeated patterns
3. **Semantic → Procedural**: Practice crystallizes knowledge into skills

Retrieval flows downward:

1. **Procedural → Semantic**: Apply learned procedures
2. **Semantic → Episodic**: Recall specific experiences
3. **Episodic → Working**: Bring relevant memories into context
