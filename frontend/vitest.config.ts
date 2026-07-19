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
      // Regression floor, not a target: set ~2% under the measured aggregate
      // (2026-07: 58.9% statements / 48.6% branches / 49.1% functions /
      // 56.5% lines). Raise the floor when coverage durably improves; never
      // lower it to admit a regression.
      thresholds: {
        statements: 57,
        branches: 47,
        functions: 47,
        lines: 54,
      },
    },
  },
});
