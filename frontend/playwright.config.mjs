import { defineConfig } from '@playwright/test';
import { viewports } from './tests/lib/env.mjs';

// Faithful to the legacy driver, the interactions lane defaults to serial: each
// scenario runs both worlds through a shared source server + oracle. Scale up
// with PARITY_WORKERS once a run is green.
const workers = Number(process.env.PARITY_WORKERS ?? 1);

function project(label) {
  const viewport = viewports.find((entry) => entry.label === label);
  if (!viewport) throw new Error(`Unknown parity viewport "${label}"`);
  return {
    name: label,
    use: {
      browserName: 'chromium',
      viewport: { width: viewport.width, height: viewport.height },
    },
  };
}

export default defineConfig({
  testDir: './tests/specs',
  globalSetup: './tests/global-setup.mjs',
  outputDir: './.cache/playwright-parity',
  fullyParallel: workers > 1,
  workers,
  retries: 0,
  reporter: [['list']],
  timeout: 120_000,
  expect: { timeout: 15_000 },
  projects: [project('desktop'), project('mobile')],
});
