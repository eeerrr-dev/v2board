import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/real-stack',
  outputDir:
    process.env.REAL_STACK_E2E_ARTIFACT_DIR ??
    './.cache/interaction-parity/real-stack-e2e',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  forbidOnly: true,
  reporter: [['list']],
  timeout: 120_000,
  expect: { timeout: 15_000 },
  use: {
    baseURL: process.env.REAL_STACK_E2E_BASE_URL ?? 'http://rust-real-stack-api:8080',
    browserName: 'chromium',
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
});
