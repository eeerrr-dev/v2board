import { defineConfig } from 'vitest/config';

// One vitest process schedules every workspace suite (user, admin, api-client)
// so `vitest run` reports them together and `--coverage` aggregates a single
// V8 report across apps and packages.
export default defineConfig({
  test: {
    projects: ['apps/*/vitest.config.ts', 'packages/*/vitest.config.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text-summary', 'html'],
    },
  },
});
