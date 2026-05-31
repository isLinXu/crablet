---
title: Creating Skills
description: Build distributable skill packages for Crablet
---

# :package: Creating Skills

Skills are the primary extension mechanism for Crablet. This guide covers creating both executable and instructional skills.

## Skill Types Comparison

| Aspect | Executable (`skill.yaml`) | Instructional (`SKILL.md`) |
|:-------|:--------------------------|:---------------------------|
| Runtime | Python/Node/Shell | None (pure prompt) |
| Complexity | Medium-High | Low |
| Flexibility | Full programming power | Behavioral instructions only |
| Distribution | Git repo or zip | Git repo or zip |
| Testing | Automated | Manual review |

## Creating an Executable Skill

### Project Structure

```
my-skill/
├── skill.yaml          # Manifest
├── main.py             # Entry point
├── requirements.txt    # Dependencies (optional)
└── README.md           # Documentation
```

### Manifest (skill.yaml)

```yaml
name: my-skill
description: What this skill does
version: 1.0.0
author: Your Name

parameters:
  type: object
  properties:
    input:
      type: string
      description: Input description
  required: [input]

entrypoint: python3 main.py
timeout: 30

# Optional environment variables
env:
  API_KEY: ${MY_API_KEY}

# Optional dependencies
dependencies:
  - name: another-skill
    version: ">=1.0.0"
```

### Entry Point

Your script receives arguments as JSON via `sys.argv[1]` and prints results to stdout:

```python
#!/usr/bin/env python3
import sys
import json

def main():
    args = json.loads(sys.argv[1])
    result = process(args["input"])
    print(json.dumps({"result": result}))

def process(input_text):
    # Your logic here
    return input_text.upper()

if __name__ == "__main__":
    main()
```

## Creating an Instructional Skill

Simply write a `SKILL.md` with frontmatter:

```markdown
---
name: rust-expert
description: Expert Rust programming assistant
version: 1.0.0
author: Your Name
tags: [coding, rust, systems-programming]
---

You are an expert Rust programmer. Follow these principles:

1. **Ownership First** — Always consider ownership and borrowing
2. **Error Handling** — Use `Result<T, E>` with `thiserror` for libraries
3. **Async** — Prefer `tokio` for async operations
4. **Testing** — Write tests for all public functions
5. **Documentation** — Add rustdoc comments to all public items

When reviewing code:
- Check for unnecessary `clone()` calls
- Verify `unsafe` blocks are justified
- Ensure proper error propagation
```

## Publishing

```bash
# Publish to a Git repository
git init
git add .
git commit -m "Initial skill release"
git remote add origin https://github.com/you/my-skill.git
git push -u origin main
```

Users can then install:

```bash
crablet skill install https://github.com/you/my-skill.git
```
