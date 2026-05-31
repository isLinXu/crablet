---
title: Skills
description: Extend Crablet with installable skill packages
---

# :package: Skills

Skills are installable packages that add new capabilities to Crablet. They come in four flavors:

## Skill Types

| Type | Format | Runtime | Use Case |
|:-----|:-------|:--------|:---------|
| **Executable** | `skill.yaml` | Python/Node/Shell | Standalone programs with I/O |
| **Instructional** | `SKILL.md` | None (pure prompt) | Behavioral instructions for the agent |
| **MCP Tool** | Config entry | External process | Remote tool servers |
| **Native Rust** | Compiled in | Rust | High-performance built-in tools |

## Installing Skills

```bash
# From a Git repository
crablet skill install https://github.com/user/weather-skill.git

# From a local directory
crablet skill install ./my-skills/calculator

# List installed skills
crablet skill list
```

## Creating an Executable Skill

### 1. Create the manifest

```yaml
# skill.yaml
name: weather
description: Get current weather for a city using OpenMeteo API
version: 1.0.0
parameters:
  type: object
  properties:
    city:
      type: string
      description: The city to get weather for
  required: [city]
entrypoint: python3 weather.py
timeout: 10
env:
  API_KEY: ${OPENMETEO_API_KEY}
```

### 2. Write the implementation

```python
# weather.py
import sys
import json
import requests

def main():
    args = json.loads(sys.argv[1])
    city = args["city"]
    
    response = requests.get(
        f"https://api.open-meteo.com/v1/forecast?city={city}"
    )
    print(json.dumps(response.json()))

if __name__ == "__main__":
    main()
```

### 3. Test locally

```bash
cd my-weather-skill
python3 weather.py '{"city": "Shanghai"}'
```

### 4. Install and use

```bash
crablet skill install .
crablet chat  # Now ask: "What's the weather in Shanghai?"
```

## Creating an Instructional Skill

```markdown
---
name: python-expert
description: Expert Python coding assistant
version: 1.0.0
---

You are a Python expert. Always use type hints and docstrings.
When writing code, follow PEP 8 conventions.
Prefer standard library solutions when available.
Always include error handling and input validation.
```

Save as `SKILL.md` and install:

```bash
crablet skill install ./python-expert
```

## Managing Skills

```bash
# List all installed skills
crablet skill list

# Remove a skill
crablet skill remove weather

# Update a skill (re-install from source)
crablet skill install --force https://github.com/user/weather-skill.git
```

## Built-in Skills

Crablet ships with two example skills:

| Skill | Description |
|:------|:------------|
| `calculator` | Mathematical calculations |
| `weather` | Weather queries via OpenMeteo |
