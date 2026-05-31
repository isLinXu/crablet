---
title: API Reference
description: Crablet's REST, WebSocket, and JSON-RPC APIs
icon: material/api
---

# :api: API Reference

Crablet exposes a unified API gateway supporting REST, WebSocket, and JSON-RPC protocols.

<div class="grid cards" markdown>

-   :globe_with_meridians: **REST API**
    
    HTTP endpoints for chat, status, and management
    
    ---
    
    [:octicons-arrow-right-24: REST API](rest.md)

-   :satellite: **WebSocket**
    
    Real-time bidirectional communication
    
    ---
    
    [:octicons-arrow-right-24: WebSocket](websocket.md)

-   :electric_plug: **JSON-RPC**
    
    Structured RPC compatible with OpenClaw protocol
    
    ---
    
    [:octicons-arrow-right-24: JSON-RPC](json-rpc.md)

</div>

## Base URL

```
http://localhost:18790
```

## Authentication

| Method | Header |
|:-------|:-------|
| API Key | `Authorization: Bearer <key>` |
| Token | `Authorization: Token <token>` |
| None | Local development only |

## Quick Reference

| Endpoint | Method | Description |
|:---------|:-------|:------------|
| `/api/chat` | POST | Send a chat message |
| `/api/status` | GET | System health status |
| `/api/skills` | GET | List installed skills |
| `/api/knowledge/query` | POST | Query knowledge base |
| `/metrics` | GET | Prometheus metrics |
