---
title: Hooks
description: Event-driven callbacks and middleware
---

# :link: Hooks

Hooks allow you to attach custom logic to events in the Crablet lifecycle — before/after tool calls, on message receipt, on errors, and more.

## Hook Points

| Hook Point | Trigger | Use Case |
|:-----------|:--------|:---------|
| `pre_tool_call` | Before a tool executes | Validate, transform, or block |
| `post_tool_call` | After a tool executes | Log, cache, post-process |
| `on_message` | Incoming user message | Filter, augment, route |
| `on_response` | Outgoing agent response | Format, censor, store |
| `on_error` | Error occurs | Alert, retry, fallback |
| `on_session_start` | New session begins | Initialize context |
| `on_session_end` | Session closes | Cleanup, persist |

## Configuration

```toml
[hooks.pre_tool_call]
# Validate all bash commands
enabled = true
script = "hooks/validate_bash.lua"

[hooks.on_error]
# Send alert on critical errors
enabled = true
script = "hooks/error_alert.lua"
```

## Example Hook Script

```lua
-- hooks/validate_bash.lua
function on_pre_tool_call(tool_name, params)
    if tool_name ~= "bash" then return nil end
    
    local cmd = params.command
    if cmd:match("rm%s+%-rf%s+/") then
        return {
            action = "block",
            reason = "Dangerous command blocked: rm -rf /"
        }
    end
    
    return { action = "allow" }
end
```
