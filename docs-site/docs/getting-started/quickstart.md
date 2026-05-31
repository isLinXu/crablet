---
title: Quickstart
description: Have your first conversation with Crablet in under 5 minutes
---

# :zap: Quickstart

This guide gets you from zero to a working Crablet conversation in under 5 minutes.

## Step 1: Set Your API Key

Crablet needs an LLM provider to function. Set at least one:

=== "OpenAI"

    ```bash
    export OPENAI_API_KEY=sk-xxx
    ```

=== "DashScope (阿里云)"

    ```bash
    export DASHSCOPE_API_KEY=sk-xxx
    ```

=== "Ollama (Local)"

    ```bash
    # Install Ollama first: https://ollama.ai
    ollama pull llama3
    
    export OLLAMA_MODEL=llama3
    ```

=== "Custom Endpoint"

    ```bash
    export OPENAI_API_BASE=https://your-api.example.com/v1
    export OPENAI_API_KEY=your-key
    ```

## Step 2: Start Chatting

=== "Interactive Chat"

    ```bash
    crablet chat
    ```
    
    ```
    🦀 Crablet v2.x — AI Agent Operating System
    
    You: Hello! What can you do?
    
    🦀 Crablet: Hi! I'm Crablet, your AI assistant. I can:
    - Answer questions and have conversations
    - Execute shell commands safely
    - Search the web
    - Read and write files
    - Run Lua scripts
    - And much more!
    
    How can I help you today?
    ```

=== "Single Command"

    ```bash
    crablet run "What's the weather in Shanghai?"
    ```

=== "Web UI"

    ```bash
    crablet serve-web --port 18790
    
    # Open http://localhost:18790 in your browser
    ```

## Step 3: Use Built-in Tools

Crablet comes with powerful built-in tools:

=== "Web Search"

    ```
    You: Search for the latest Rust release
    
    🦀 Crablet: [Calling tool: web_search]
    The latest Rust release is 1.82.0, released on October 17, 2024...
    ```

=== "File Operations"

    ```
    You: Read the file /tmp/notes.md
    
    🦀 Crablet: [Calling tool: file_read]
    Here's the content of /tmp/notes.md:
    ...
    ```

=== "Shell Commands"

    ```
    You: List all Python files in the current directory
    
    🦀 Crablet: [Calling tool: bash]
    Found 3 Python files:
    - main.py
    - utils.py
    - config.py
    ```

## Step 4: Install a Skill

```bash
# Browse available skills
crablet skill list

# Install a skill from a Git repository
crablet skill install https://github.com/user/weather-skill.git

# Create your own skill
crablet skill create my-custom-skill
```

## Step 5: Explore More

Now that you're up and running, here's where to go next:

- [:gear: Configuration](configuration.md) — Customize model, safety, and channel settings
- [:brain: Cognitive System](../user-guide/cognitive/index.md) — Understand the three-layer thinking architecture
- [:package: Skills](../user-guide/features/skills.md) — Extend Crablet with installable skill packages
- [:map: Learning Path](learning-path.md) — Find the guided path for your goals
