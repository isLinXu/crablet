---
name: safe_run
version: 1.0.0
description: Execute shell commands securely inside a Docker sandbox
author: Crablet
tags: [security, execution, docker]
---

# Safe Run Skill

This skill executes shell commands inside an isolated Docker container to prevent harm to the host system.

## Tools

### `run_command`
Executes a command in a sandboxed environment.

- **command** (string): The shell command to execute.

## Implementation

```python
import subprocess
import shlex

def run_command(command: str) -> str:
    """
    Executes a command inside an ephemeral Docker container (alpine).
    The container is removed immediately after execution (--rm).
    Network access is disabled by default (--network none) for security,
    unless explicitly needed (can be configured).
    """
    
    # Security: Use shlex to quote the user command to prevent injection outside the container command
    # However, since we are passing the whole string to /bin/sh -c inside docker, 
    # we need to be careful. Ideally we pass it as a single argument.
    
    # Docker command structure:
    # docker run --rm --network none alpine /bin/sh -c "command"
    
    docker_cmd = [
        "docker", "run", "--rm",
        "--network", "none",  # Disable network for SSRF prevention
        "--memory", "128m",   # Limit memory
        "--cpus", "0.5",      # Limit CPU
        "alpine:latest",      # Use lightweight image
        "/bin/sh", "-c", command
    ]
    
    try:
        # Run docker command
        result = subprocess.run(
            docker_cmd,
            capture_output=True,
            text=True,
            timeout=30 # Hard timeout of 30 seconds
        )
        
        if result.returncode != 0:
            return f"Error (Exit Code {result.returncode}):\n{result.stderr}"
            
        return result.stdout if result.stdout else "(No output)"
        
    except subprocess.TimeoutExpired:
        return "Error: Command execution timed out (30s limit)."
    except Exception as e:
        return f"System Error: {str(e)}"

# Register the tool
register_tool("run_command", run_command)
```
