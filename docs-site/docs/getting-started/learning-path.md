---
title: Learning Path
description: Find the right learning path based on your experience level and goals
---

# :map: Learning Path

Crablet can do a lot — CLI assistant, Telegram/Discord bot, task automation, multi-agent coordination, and more. This page helps you figure out where to start and what to read based on your experience level and what you're trying to accomplish.

!!! tip "Start Here"
    If you haven't installed Crablet yet, begin with the [Installation guide](installation.md) and then run through the [Quickstart](quickstart.md). Everything below assumes you have a working installation.

## How to Use This Page

- **Know your level?** Jump to the [experience-level table](#by-experience-level) and follow the reading order for your tier.
- **Have a specific goal?** Skip to [By Use Case](#by-use-case) and find the scenario that matches.
- **Just browsing?** Check the [Key Features](../index.md#key-features-at-a-glance) table for a quick overview of everything Crablet can do.

## By Experience Level

| Level | Goal | Recommended Reading | Time |
|:------|:-----|:--------------------|:-----|
| **Beginner** | Get up and running, have basic conversations, use built-in tools | [Installation](installation.md) → [Quickstart](quickstart.md) → [CLI Usage](../user-guide/cli.md) → [Configuration](configuration.md) | ~1 hour |
| **Intermediate** | Set up messaging bots, use advanced features like memory, cron, skills | [Tools](../user-guide/features/tools.md) → [Messaging](../user-guide/messaging/index.md) → [Skills](../user-guide/features/skills.md) → [Memory](../user-guide/memory/index.md) → [Cron](../user-guide/features/cron.md) | ~2–3 hours |
| **Advanced** | Build custom tools, create skills, understand cognitive architecture, contribute | [Architecture](../developer-guide/architecture.md) → [Adding Tools](../developer-guide/adding-tools.md) → [Creating Skills](../developer-guide/creating-skills.md) → [Cognitive System](../user-guide/cognitive/index.md) → [Contributing](../developer-guide/contributing.md) | ~4–6 hours |

## By Use Case

### :speech_balloon: "I want a CLI coding assistant"

Use Crablet as an interactive terminal assistant for writing, reviewing, and running code.

1. [Installation](installation.md)
2. [Quickstart](quickstart.md)
3. [CLI Usage](../user-guide/cli.md)
4. [Code Execution](../user-guide/features/code-execution.md)
5. [Knowledge & RAG](../user-guide/features/knowledge.md)
6. [Configuration](configuration.md)

!!! tip "Pro Tip"
    Feed files directly into your conversation with context files. Crablet can read, edit, and run code in your projects.

### :robot: "I want a Telegram/Discord bot"

Deploy Crablet as a bot on your favorite messaging platform.

1. [Installation](installation.md)
2. [Configuration](configuration.md)
3. [Messaging Overview](../user-guide/messaging/index.md)
4. [Telegram Setup](../user-guide/messaging/telegram.md)
5. [Discord Setup](../user-guide/messaging/discord.md)
6. [Security](../user-guide/security.md)

### :repeat: "I want to automate tasks"

Schedule recurring tasks, run batch jobs, or chain agent actions together.

1. [Quickstart](quickstart.md)
2. [Cron Scheduling](../user-guide/features/cron.md)
3. [Lua Scripting](../user-guide/features/code-execution.md)
4. [Delegation](../user-guide/features/delegation.md)
5. [Hooks](../user-guide/features/hooks.md)

!!! tip "Automation Tip"
    Cron jobs let Crablet run tasks on a schedule — daily summaries, periodic checks, automated reports — without you being present.

### :wrench: "I want to build custom tools/skills"

Extend Crablet with your own tools and reusable skill packages.

1. [Tools Overview](../user-guide/features/tools.md)
2. [Skills Overview](../user-guide/features/skills.md)
3. [Adding Tools](../developer-guide/adding-tools.md)
4. [Creating Skills](../developer-guide/creating-skills.md)
5. [Architecture](../developer-guide/architecture.md)
6. [MCP Protocol](../user-guide/features/tools.md#mcp-support)

### :brain: "I want to leverage the cognitive system"

Understand and configure Crablet's three-layer thinking architecture.

1. [Quickstart](quickstart.md)
2. [Three-Layer Architecture](../user-guide/cognitive/three-layer.md)
3. [Meta-Cognition](../user-guide/cognitive/meta-cognition.md)
4. [Thought Visualization](../user-guide/cognitive/thought-visualization.md)
5. [Architecture Deep Dive](../developer-guide/architecture.md)

### :whale: "I want to deploy to production"

Set up Crablet for production use with monitoring and scaling.

1. [Deployment Overview](../deployment/index.md)
2. [Docker Setup](../deployment/docker.md)
3. [Production Config](../deployment/production.md)
4. [Monitoring](../deployment/monitoring.md)
5. [Security](../user-guide/security.md)

## What to Read Next

Based on where you are right now:

- **Just finished installing?** → Head to the [Quickstart](quickstart.md) to run your first conversation.
- **Completed the Quickstart?** → Read [CLI Usage](../user-guide/cli.md) and [Configuration](configuration.md) to customize your setup.
- **Comfortable with the basics?** → Explore [Tools](../user-guide/features/tools.md), [Skills](../user-guide/features/skills.md), and [Memory](../user-guide/memory/index.md) to unlock the full power of the agent.
- **Setting up for a team?** → Read [Security](../user-guide/security.md) and [Deployment](../deployment/index.md) to understand access control and production setup.
- **Ready to build?** → Jump into the [Developer Guide](../developer-guide/index.md) to understand the internals and start contributing.
