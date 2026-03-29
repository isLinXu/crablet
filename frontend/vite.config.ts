/// <reference types="vitest" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const dirname = path.dirname(fileURLToPath(import.meta.url))

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:18790',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://127.0.0.1:18790',
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
