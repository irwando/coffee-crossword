import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    // Use 127.0.0.1 explicitly — on macOS, 'localhost' resolves to IPv6 ::1
    // first, causing a TCP connection delay on every module request from WebKit.
    host: '127.0.0.1',
    port: 5173,
    // Pre-warm entry points so the first page load doesn't stall on transforms.
    warmup: {
      clientFiles: ['./src/main.tsx', './src/App.tsx', './src/ResultsColumn.tsx', './src/WordListDrawer.tsx'],
    },
  },
})
