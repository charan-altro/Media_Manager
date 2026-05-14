import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import type { UserConfig } from 'vite'

interface VitestConfigExport extends UserConfig {
  test: any;
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    tailwindcss(),
    react()
  ],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:7878',
        changeOrigin: true,
      },
    },
  },
} as VitestConfigExport)
