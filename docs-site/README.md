# Crablet Documentation Site

This directory contains the source for the Crablet documentation site, built with [MkDocs Material](https://squidfunk.github.io/mkdocs-material/).

## Local Development

### Prerequisites

```bash
pip install mkdocs-material pymdown-extensions mkdocs-minify-plugin mkdocs-git-revision-date-localized-plugin
```

### Serve Locally

```bash
cd docs-site
mkdocs serve
# Open http://localhost:8000
```

### Build Static Site

```bash
cd docs-site
mkdocs build
# Output in docs-site/site/
```

## Deploy to GitHub Pages

The site is automatically deployed via GitHub Actions on push to `main`.

See `.github/workflows/docs.yml` for the deployment workflow.

## Directory Structure

```
docs-site/
├── mkdocs.yml                  # Site configuration
├── docs/
│   ├── index.md                # Homepage
│   ├── getting-started/        # Installation, Quickstart, Learning Path, Config
│   ├── user-guide/             # CLI, Security
│   │   ├── features/           # Tools, Skills, Knowledge, Browser, etc.
│   │   ├── messaging/          # Telegram, Discord, Web UI
│   │   ├── cognitive/          # Three-Layer, Meta-Cognition, Visualization
│   │   └── memory/             # Four-Layer, Fusion
│   ├── developer-guide/        # Architecture, Adding Tools/Skills/Channels
│   ├── guides/                 # Practical tutorials
│   ├── deployment/             # Docker, Production, Monitoring
│   ├── api/                    # REST, WebSocket, JSON-RPC
│   ├── faq/                    # FAQ
│   ├── includes/               # Abbreviations, reusable snippets
│   └── assets/                 # CSS, JS, SVG
└── README.md                   # This file
```

## Contributing to Docs

1. Edit or add Markdown files in `docs/`
2. Update `mkdocs.yml` nav if adding new pages
3. Preview with `mkdocs serve`
4. Submit a PR
