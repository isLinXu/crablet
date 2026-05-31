---
title: WebSocket
description: Real-time bidirectional communication
---

# :satellite: WebSocket API

Connect to `ws://localhost:18789/ws` for real-time bidirectional communication.

## Connection

```javascript
const ws = new WebSocket('ws://localhost:18789/ws');

ws.onopen = () => {
  console.log('Connected to Crablet');
};
```

## Client → Server Messages

### Send a message

```json
{
  "type": "UserInput",
  "content": "Hello, Crablet!"
}
```

### Switch model

```json
{
  "type": "SwitchModel",
  "model": "gpt-4o"
}
```

### Toggle tools

```json
{
  "type": "ToggleTools",
  "enabled": true
}
```

## Server → Client Events

| Event Type | Description |
|:-----------|:------------|
| `ThoughtGenerated` | ReAct thinking step |
| `ToolExecutionStarted` | Tool call initiated |
| `ToolExecutionFinished` | Tool call completed |
| `ResponseGenerated` | Final response ready |
| `SwarmActivity` | Multi-agent coordination event |
| `CognitiveSwitch` | Cognitive layer changed |
| `Error` | Error occurred |

### Example: Thought Generated

```json
{
  "type": "ThoughtGenerated",
  "layer": "system2",
  "step": 1,
  "content": "Analyzing the user's request...",
  "confidence": 0.85
}
```

### Example: Tool Execution

```json
{
  "type": "ToolExecutionStarted",
  "tool": "bash",
  "params": {"command": "ls -la"}
}
```

```json
{
  "type": "ToolExecutionFinished",
  "tool": "bash",
  "output": "total 42\ndrwxr-xr-x...",
  "duration_ms": 45
}
```
