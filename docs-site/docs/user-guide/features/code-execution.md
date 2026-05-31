---
title: Code Execution
description: Lua scripting engine for extensible automation
---

# :code_block: Code Execution

Crablet embeds a Lua 5.4 scripting engine for extensible automation directly within the agent.

## Running Scripts

```bash
# Execute a Lua script
crablet script run examples/scripts/summarize_paper.lua

# With arguments
crablet script run analyze.lua --args '{"input": "data.csv"}'
```

## Example: Summarize a Paper

```lua
-- summarize_paper.lua
local paper = io.read_file(args.input_path)
local summary = llm.chat({
    model = "gpt-4o-mini",
    prompt = "Summarize this paper concisely:\n" .. paper
})
io.write_file(args.output_path, summary)
print("Summary written to " .. args.output_path)
```

## Available Lua APIs

| Module | Functions |
|:-------|:----------|
| `io` | `read_file`, `write_file`, `list_dir` |
| `llm` | `chat`, `embed`, `count_tokens` |
| `http` | `get`, `post`, `put`, `delete` |
| `json` | `encode`, `decode` |
| `math` | Standard Lua math + extras |
| `os` | `exec`, `env`, `time` |

## Safety

All Lua scripts run within the Safety Oracle's constraints:

- File access is path-restricted
- Shell commands require approval in Strict mode
- Resource limits (CPU time, memory) are enforced
