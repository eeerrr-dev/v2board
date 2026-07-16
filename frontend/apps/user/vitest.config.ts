import { defineConfig } from 'vitest/config';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import path from 'node:path';

export default defineConfig({
  // Mirror vite.config.ts: unit tests must exercise the same React Compiler
  // output that dev and deploy builds ship, not unmemoized variants.
  plugins: [react(), babel({ presets: [reactCompilerPreset()] })],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, 'src') } },
  test: {
    environment: 'happy-dom',
    globals: false,
    include: ['src/**/*.test.{ts,tsx}'],
    setupFiles: ['src/test/setup-local-storage.ts', 'src/test/setup-testing-library.ts'],
  },
});
