/// <reference types="vitest" />
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const dirname = path.dirname(fileURLToPath(import.meta.url))
const semverPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/

export function resolveCrabletVersion(env = process.env): string {
  const override = env.CRABLET_VERSION?.trim().replace(/^v/, '')
  const cargoToml = fs.readFileSync(path.resolve(dirname, '../crablet/Cargo.toml'), 'utf8')
  const sourceVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1]
  const version = override || sourceVersion

  if (!version || !semverPattern.test(version)) {
    throw new Error(`Invalid Crablet version: ${version || '<missing>'}`)
  }
  return version
}

// https://vite.dev/config/
export default defineConfig({
  define: {
    __CRABLET_VERSION__: JSON.stringify(resolveCrabletVersion()),
  },
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:18799',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://127.0.0.1:18799',
        ws: true,
      }
    }
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return
          }
          if (
            id.includes('d3')
          ) {
            return 'd3-vendor'
          }
          if (
            id.includes('tesseract.js')
          ) {
            return 'ocr-vendor'
          }
          if (
            id.includes('pdfjs-dist')
          ) {
            return 'pdf-vendor'
          }
          if (
            id.includes('chart.js') ||
            id.includes('react-chartjs-2')
          ) {
            return 'chart-vendor'
          }
          if (
            id.includes('@xyflow/react') ||
            id.includes('dagre')
          ) {
            return 'flow-vendor'
          }
          if (
            id.includes('react-markdown') ||
            id.includes('react-virtuoso')
          ) {
            return 'chat-vendor'
          }
          if (
            id.includes('react-router-dom') ||
            id.includes('react-dom') ||
            id.includes('react') ||
            id.includes('zustand')
          ) {
            return 'react-core'
          }
          if (
            id.includes('i18next') ||
            id.includes('react-i18next')
          ) {
            return 'i18n-vendor'
          }
          if (id.includes('axios')) {
            return 'http-vendor'
          }
          if (id.includes('lucide-react')) {
            return 'icons-vendor'
          }
        },
      },
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    coverage: {
      provider: 'v8',
      reporter: ['text-summary', 'lcov', 'html'],
      reportsDirectory: './coverage',
      exclude: [
        'src/test/**',
        '**/*.d.ts',
      ],
    },
  },
})
