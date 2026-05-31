---
title: REST API
description: HTTP endpoints for chat, status, and management
---

# :globe_with_meridians: REST API

## Chat

### POST /api/chat

Send a message and receive a response.

```json
// Request
{
  "message": "Explain Rust ownership",
  "model": "gpt-4o-mini",
  "stream": false,
  "tools": true,
  "session_id": "optional-session-id"
}

// Response
{
  "content": "Rust ownership is...",
  "model": "gpt-4o-mini",
  "tokens_used": 245,
  "duration_ms": 1200,
  "cognitive_layer": "system2",
  "confidence": 0.94
}
```

### POST /api/chat/stream

Stream a response with Server-Sent Events.

```json
// Request (same as above with stream: true)
{
  "message": "Explain Rust ownership",
  "stream": true
}

// SSE Events
event: thought
data: {"layer": "system2", "step": 1, "content": "Searching documentation..."}

event: tool_call
data: {"tool": "bash", "params": {"command": "rustc --version"}}

event: token
data: {"content": "Rust"}

event: token
data: {"content": " ownership"}

event: done
data: {"tokens_used": 245, "duration_ms": 1200}
```

## Status

### GET /api/status

```json
{
  "version": "2.0.0",
  "uptime_seconds": 3600,
  "sessions_active": 5,
  "models_available": ["gpt-4o-mini", "gpt-4o"],
  "tools_registered": 9,
  "skills_installed": 3,
  "memory_stats": {
    "working_items": 42,
    "episodic_count": 150,
    "semantic_entities": 89
  }
}
```

## Skills

### GET /api/skills

```json
[
  {
    "name": "weather",
    "version": "1.0.0",
    "description": "Weather queries via OpenMeteo",
    "type": "executable"
  }
]
```

## Knowledge

### POST /api/knowledge/query

```json
// Request
{
  "query": "What is the attention mechanism?",
  "top_k": 5,
  "include_sources": true
}

// Response
{
  "results": [
    {
      "content": "The attention mechanism...",
      "score": 0.92,
      "source": "papers/attention_is_all_you_need.pdf",
      "page": 3
    }
  ]
}
```
