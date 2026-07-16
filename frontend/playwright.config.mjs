import { defineConfig } from '@playwright/test';
import { viewports } from './tests/lib/env.mjs';

// Faithful to the legacy driver, the interactions lane defaults to serial: each
// scenario runs both worlds through a shared source server + oracle. Scale up
// with PARITY_WORKERS once a run is green.
const workers = Number(process.env.PARITY_WORKERS) || 1;

// VISUAL_PARITY_VIEWPORTS selects which viewport projects run (default both).
// The Makefile / rust-interaction-parity pass a subset here.
const requestedViewports = (process.env.VISUAL_PARITY_VIEWPORTS ?? 'desktop mobile')
  .trim()
  .split(/\s+/)
  .filter(Boolean);

// INTERACTION_PARITY_SCENARIOS narrows the run to specific interaction labels
// (empty = run every interaction). Each label is anchored to the end of the test
// title so e.g. `...coupon` never also matches `...coupon-error`.
const scenarioFilter = (process.env.INTERACTION_PARITY_SCENARIOS ?? '').trim();
const grep = scenarioFilter
  ? new RegExp(
      '(?:' +
        scenarioFilter
          .split(/\s+/)
          .filter(Boolean)
          .map((label) => `${label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`)
          .join('|') +
        ')',
    )
  : undefined;

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
  outputDir: process.env.INTERACTION_PARITY_ARTIFACT_DIR ?? './.cache/playwright-parity',
  fullyParallel: workers > 1,
  workers,
  // One retry absorbs the reference oracle's animation-timing flakes (its antd
  // overlays can miss a tight close wait under full-suite load) while keeping
  // the gate strict: a real contract regression is deterministic and fails both
  // attempts. The first attempt runs untraced — always-on trace recording added
  // enough Chromium overhead to tip those oracle waits over the edge — and the
  // retry records the full trace for diagnosis.
  retries: 1,
  // Unconditional: the Docker gate does not forward CI into the container, and
  // local narrowing goes through INTERACTION_PARITY_SCENARIOS, never test.only.
  // A committed .only would otherwise silently shrink the whole contract gate.
  forbidOnly: true,
  reporter: [['list']],
  timeout: 120_000,
  expect: { timeout: 15_000 },
  grep,
  use: {
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: requestedViewports.map(project),
});
