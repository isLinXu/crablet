---
title: JSON-RPC
description: Structured RPC compatible with OpenClaw protocol
---

# :electric_plug: JSON-RPC API

Crablet supports JSON-RPC 2.0 for structured remote procedure calls, compatible with the OpenClaw protocol.

## Endpoint

```
POST /rpc
Content-Type: application/json
```

## Methods

### chat.send

Send a chat message and receive a response.

```json
{
  "jsonrpc": "2.0",
  "method": "chat.send",
  "params": {
    "message": "Hello!",
    "model": "gpt-4o-mini",
    "stream": false
  },
  "id": 1
}
```

### status.get

Retrieve system status.

```json
{
  "jsonrpc": "2.0",
  "method": "status.get",
  "id": 2
}
```

### skill.list

List installed skills.

```json
{
  "jsonrpc": "2.0",
  "method": "skill.list",
  "id": 3
}
```

### knowledge.query

Query the knowledge base.

```json
{
  "jsonrpc": "2.0",
  "method": "knowledge.query",
  "params": {
    "query": "attention mechanism",
    "top_k": 5
  },
  "id": 4
}
```

## Error Codes

| Code | Meaning |
|:-----|:--------|
| `-32700` | Parse error |
| `-32600` | Invalid request |
| `-32601` | Method not found |
| `-32602` | Invalid params |
| `-32603` | Internal error |
| `-32001` | LLM provider error |
| `-32002` | Safety violation |
| `-32003` | Rate limit exceeded |
