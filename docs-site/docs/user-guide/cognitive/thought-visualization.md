---
title: Thought Visualization
description: Real-time rendering of cognitive processes
---

# :chart_line: Thought Visualization

Crablet's Web UI renders the cognitive process in real-time, giving you insight into how the agent thinks.

## Visualization Components

### Cognitive Load Indicators

Real-time bars showing processing intensity for each system:

- **System 1 Load** — Typically 5-20% (background intuition)
- **System 2 Load** — Spikes to 70-98% during reasoning
- **System 3 Load** — Activated during reflection (30-60%)

### Reasoning Chain

Step-by-step display of System 2's ReAct loop:

```
🔍 Step 1: Searching for relevant documentation...
🔍 Step 2: Reading API reference...
🔍 Step 3: Comparing implementations...
🔍 Step 4: Synthesizing findings...
✅ Confidence: 94%
```

### Meta-Cognitive Panel

Shows System 3's self-reflection when activated:

- Problem diagnosis
- Root cause analysis
- Improvement suggestions
- Strategy adjustments

## Frontend Components

The visualization is built with React + TailwindCSS:

| Component | Location |
|:----------|:---------|
| `EnhancedThinkingVisualization` | `frontend/src/components/chat/` |
| `ActionableSmartSuggestions` | `frontend/src/components/cognitive/` |
| Cognitive dashboard widgets | `frontend/src/components/dashboard/` |

## Controlling the Display

The thought panel defaults to collapsed for clean UX:

- **Collapsed**: Shows current cognitive layer label only
- **Expanded**: Full reasoning chain, load meters, controls, suggestions

Smart suggestions are context-aware:

- Code contexts → Code review, optimization suggestions
- Data contexts → Analysis, visualization suggestions
- Casual conversation → Hidden (no noise)
