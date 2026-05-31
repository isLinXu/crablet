---
title: Home
description: Crablet — A Production-Ready AI Agent Operating System Built from Scratch in Rust
hide:
  - navigation
  - toc
---

<!-- Hero Banner -->
<div class="hero-section" markdown>

# :crab: Crablet

**A Production-Ready AI Agent Operating System Built from Scratch in Rust**

*"Making AI as ubiquitous as water — fluid, adaptive, and ceaseless."*

[:rocket: Quick Start](getting-started/quickstart.md){ .md-button .md-button--primary }
[:book: Learning Path](getting-started/learning-path.md){ .md-button }
[:fontawesome-brands-github: GitHub](https://github.com/isLinXu/crablet){ .md-button }

</div>

---

## :rocket: Where to Start

<div class="grid cards" markdown>

-   :hammer_and_wrench: **Getting Started**
    
    Install Crablet and get up and running in minutes
    
    ---
    
    [:octicons-arrow-right-24: Installation](getting-started/installation.md)

-   :speech_balloon: **Chat with Crablet**
    
    Interactive CLI assistant for writing, reviewing, and running code
    
    ---
    
    [:octicons-arrow-right-24: CLI Usage](user-guide/cli.md)

-   :brain: **Cognitive System**
    
    Three-layer cognitive architecture with meta-cognition and self-improvement
    
    ---
    
    [:octicons-arrow-right-24: Learn More](user-guide/cognitive/index.md)

-   :gear: **Configure & Deploy**
    
    Docker, Kubernetes, production best practices
    
    ---
    
    [:octicons-arrow-right-24: Deployment Guide](deployment/index.md)

</div>

---

## :star: Key Features at a Glance

<div class="grid cards" markdown>

-   :wrench: **Tools** <span class="feature-badge stable">Stable</span>
    
    Built-in tools the agent can call — bash, file I/O, web search, HTTP, calculator, and more
    
    ---
    
    [:octicons-arrow-right-24: Tools](user-guide/features/tools.md)

-   :package: **Skills** <span class="feature-badge stable">Stable</span>
    
    Installable plugin packages that add new capabilities via CLI
    
    ---
    
    [:octicons-arrow-right-24: Skills](user-guide/features/skills.md)

-   :brain: **Three-Layer Cognition** <span class="feature-badge new">New</span>
    
    System 1 (fast) → System 2 (slow reasoning) → System 3 (meta-cognition) architecture
    
    ---
    
    [:octicons-arrow-right-24: Cognitive System](user-guide/cognitive/index.md)

-   :floppy_disk: **Four-Layer Memory** <span class="feature-badge new">New</span>
    
    Working → Episodic → Semantic → Procedural memory with fusion retrieval
    
    ---
    
    [:octicons-arrow-right-24: Memory System](user-guide/memory/index.md)

-   :mag: **Knowledge & RAG** <span class="feature-badge beta">Beta</span>
    
    Vector search, graph RAG, entity extraction, and hybrid retrieval
    
    ---
    
    [:octicons-arrow-right-24: Knowledge](user-guide/features/knowledge.md)

-   :robot: **Multi-Agent Swarm** <span class="feature-badge beta">Beta</span>
    
    Spawn sub-agents for parallel work with hierarchical coordination
    
    ---
    
    [:octicons-arrow-right-24: Delegation](user-guide/features/delegation.md)

-   :globe_with_meridians: **Multi-Channel** <span class="feature-badge stable">Stable</span>
    
    Unified channel abstraction: CLI, Web UI, Telegram, Discord, and more
    
    ---
    
    [:octicons-arrow-right-24: Messaging](user-guide/messaging/index.md)

-   :shield: **Safety Oracle** <span class="feature-badge stable">Stable</span>
    
    Command validation, path sandboxing, and configurable security policies
    
    ---
    
    [:octicons-arrow-right-24: Security](user-guide/security.md)

-   :clock4: **Cron Scheduling** <span class="feature-badge beta">Beta</span>
    
    Schedule recurring agent tasks with flexible cron expressions
    
    ---
    
    [:octicons-arrow-right-24: Cron](user-guide/features/cron.md)

-   :earth_americas: **Browser Automation** <span class="feature-badge beta">Beta</span>
    
    Headless browser for web scraping and interaction
    
    ---
    
    [:octicons-arrow-right-24: Browser](user-guide/features/browser.md)

-   :memo: **Lua Scripting** <span class="feature-badge stable">Stable</span>
    
    Embedded Lua 5.4 scripting engine for extensible automation
    
    ---
    
    [:octicons-arrow-right-24: Code Execution](user-guide/features/code-execution.md)

-   :chart_line: **Observability** <span class="feature-badge beta">Beta</span>
    
    OpenTelemetry tracing, Prometheus metrics, and Jaeger integration
    
    ---
    
    [:octicons-arrow-right-24: Monitoring](deployment/monitoring.md)

</div>

---

## :crab: Why Crablet?

Crablet is built from scratch in Rust, delivering unmatched performance and reliability for AI agent infrastructure:

| Aspect | Benefit |
|:-------|:--------|
| :zap: **Performance** | Native Rust performance — 3-5× faster than Node.js alternatives |
| :shield: **Safety** | Compile-time guarantees, no null pointers, no data races |
| :recycle: **Concurrency** | Tokio async runtime supporting 100+ concurrent agents |
| :battery: **Reliability** | Type-safe error handling with anyhow + thiserror |
| :electric_plug: **Extensibility** | Plugin architecture with Skills, Tools, MCP, and Lua scripting |
| :whale: **Deployment** | Docker images < 20 MB, startup < 500 ms |

---

## :octicons-code-24: Quick Example

=== "CLI Chat"

    ```bash
    # Interactive conversation
    crablet chat
    
    # Single command execution
    crablet run "Summarize the latest news about Rust"
    ```

=== "Web UI"

    ```bash
    # Start unified Web UI + API gateway
    crablet serve-web --port 18790
    
    # Or explicit gateway mode
    crablet gateway --port 18790
    ```

=== "Docker"

    ```bash
    docker run -d \
      --name crablet \
      -p 18790:18790 \
      -e OPENAI_API_KEY=sk-xxx \
      crablet:latest
    ```

=== "Rust API"

    ```rust
    use crablet::prelude::*;
    
    #[tokio::main]
    async fn main() -> Result<()> {
        let mut agent = Crablet::new(config?).await?;
        
        let response = agent
            .chat("Explain Rust ownership")
            .with_model("gpt-4o")
            .with_tools(true)
            .send()
            .await?;
        
        println!("{}", response.content);
        Ok(())
    }
    ```

---

## :bulb: What to Read Next

Based on where you are right now:

- **Just heard about Crablet?** → Head to [:rocket: Installation](getting-started/installation.md) to set up your environment.
- **Already installed?** → Run through the [:book: Quickstart](getting-started/quickstart.md) to have your first conversation.
- **Looking for a guided path?** → Check the [:map: Learning Path](getting-started/learning-path.md) tailored to your experience level.
- **Want to build something?** → Jump into [:wrench: Developer Guide](developer-guide/index.md) to understand the internals.
- **Deploying to production?** → Read [:whale: Deployment](deployment/index.md) for Docker and monitoring setup.
