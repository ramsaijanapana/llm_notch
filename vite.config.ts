import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    // Bind IPv4 explicitly so Tauri's 127.0.0.1:1420 probe succeeds.
    // `localhost` can resolve to ::1 on macOS and leave tauri waiting forever.
    port: 1420,
    strictPort: true,
    host: host || '127.0.0.1',
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Ignore Rust build outputs — watching locked .dll/.exe on Windows
      // races cargo and crashes Vite with EBUSY.
      ignored: ['**/src-tauri/**', '**/target/**'],
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    css: true,
  },
})
