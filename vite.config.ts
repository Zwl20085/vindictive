/// <reference types="vitest" />
import { defineConfig } from 'vite';
import { fileURLToPath, URL } from 'node:url';

const page = (name: string): string => fileURLToPath(new URL(name, import.meta.url));

const DEV_PORT = 1420;
const COVERAGE_THRESHOLD = 80;

export default defineConfig({
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  server: {
    port: DEV_PORT,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    target: 'es2021',
    rollupOptions: {
      input: {
        main: page('index.html'),
        capture: page('capture.html'),
      },
    },
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/lib/**/*.ts'],
      exclude: ['src/lib/**/*.test.ts'],
      thresholds: { lines: COVERAGE_THRESHOLD },
    },
  },
});
