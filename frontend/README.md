# Crablet Frontend

This is the visualization and debugging frontend for Crablet.

## Setup

```bash
npm install
```

## Development

```bash
npm run dev
```

The frontend resolves all backend traffic from a single configured API base URL. In local development the default is the unified gateway on `http://127.0.0.1:18790/api`, while Vite dev uses the `/api` proxy path.

## Validation

```bash
npm ci
npm run type-check
npm run test:ci -- constants
```

## Features (Planned)

- **Canvas**: Visual agent flow editor.
- **Debug**: Real-time event log and trace viewer.
- **Chat**: Rich chat interface with markdown/image support.
