/// <reference types="vitest" />
import { defineConfig } from 'vite';
import { fileURLToPath, URL } from 'node:url';

const page = (name: string): string => fileURLToPath(new URL(name, import.meta.url));

const DEV_PORT = 1420;
// Pure logic in src/lib must stay well covered. The DOM layer in src/ui is
// mostly exercised by hand in the running app, so its floor is a ratchet set
// about five points below the measured value (32% lines in 0.4.0): raise it
// as ui tests are added, never lower it.
const LIB_LINES_THRESHOLD = 80;
const UI_LINES_THRESHOLD = 77;

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
      include: ['src/lib/**/*.ts', 'src/ui/**/*.ts'],
      exclude: ['src/**/*.test.ts'],
      thresholds: {
        'src/lib/**/*.ts': { lines: LIB_LINES_THRESHOLD },
        'src/ui/**/*.ts': { lines: UI_LINES_THRESHOLD },
      },
    },
  },
});
