---
title: Browser Automation
description: Headless browser for web scraping and interaction
---

# :globe_with_meridians: Browser Automation

Crablet includes a headless browser tool powered by Chromium for web scraping and interaction.

## Basic Usage

```
You: Scrape the headlines from https://news.ycombinator.com

🦀 Crablet: [Calling tool: browser]
Here are the top headlines from Hacker News:
1. Show HN: I built a distributed database in Rust
2. Understanding memory allocation in Linux
3. ...
```

## Capabilities

- **Page Navigation** — Visit URLs and follow links
- **Content Extraction** — Extract text, tables, and structured data
- **Form Interaction** — Fill forms, click buttons, submit data
- **Screenshots** — Capture page snapshots
- **JavaScript Execution** — Run custom JS on pages

## Configuration

```toml
[browser]
headless = true
timeout = 30
user_agent = "Mozilla/5.0 (compatible; Crablet/1.0)"
viewport_width = 1920
viewport_height = 1080
```

!!! warning "Security Note"
    Browser automation is disabled by default in Strict safety mode. Enable it explicitly in Permissive mode.
