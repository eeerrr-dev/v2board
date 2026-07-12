// Dual-world interaction harness.
//
// One run(page) drives both worlds: the shadcn source build (served live at
// sourceBaseUrl) and the frozen antd oracle (served by global-setup's in-process
// server). Each world independently asserts its raw result is useful
// (assertUsefulInteraction), then the results are reduced to Tier-1 fields
// (normalizeInteractionResult + collapseCjkDeep) and compared cross-world by
// stableJson equality. This ports the interactions lane of the retired
// frontend/scripts/visual-parity.mjs driver (runInteractionParity /
// runInteractionTarget / preparePageForInteraction) onto @playwright/test.
import { readFileSync } from 'node:fs';
import { sourceBaseUrl, viewports } from './env.mjs';
import { collapseCjkDeep } from './text.mjs';
import { stableJson } from './json-util.mjs';
import { getScenario } from './scenario-meta.mjs';
import { normalizeInteractionResult } from './normalizers.mjs';
import { assertUsefulInteraction } from './assert-useful.mjs';
import { installApiFixtures, seedLegacyAdminStore, delay } from './api-fixtures.mjs';
import { readDebugSnapshot, waitForReadySelector } from './dom-helpers.mjs';
import {
  gotoStable,
  navigateAfterWarmup,
  waitForMountedContent,
  waitForFontsBeforeCapture,
  waitForFixedColumnLayout,
} from './page-prep.mjs';
import { oracleUrlFile } from './oracle-url.mjs';

let cachedOracleBaseUrl;

function oracleBaseUrl() {
  if (!cachedOracleBaseUrl) {
    let raw;
    try {
      raw = readFileSync(oracleUrlFile, 'utf8').trim();
    } catch {
      throw new Error(
        `Oracle base URL file missing (${oracleUrlFile}). ` +
          'The Playwright globalSetup must run before any parity spec.',
      );
    }
    if (!raw) throw new Error(`Oracle base URL file is empty (${oracleUrlFile}).`);
    cachedOracleBaseUrl = new URL(raw);
  }
  return cachedOracleBaseUrl;
}

export function viewportByLabel(label) {
  const viewport = viewports.find((entry) => entry.label === label);
  if (!viewport) throw new Error(`Unknown parity viewport "${label}"`);
  return viewport;
}

// Mirror runInteractionParity's per-interaction viewport gate: an interaction
// with an explicit `viewports` list runs only on those; otherwise on all.
export function interactionAppliesToViewport(interaction, projectName) {
  return !interaction.viewports || interaction.viewports.includes(projectName);
}

// Port of preparePageForInteraction(page, url, scenario, target, interaction).
async function preparePageForInteraction(page, url, scenario, target, interaction = {}) {
  const diagnostics = [];
  page.__visualParityDiagnostics = diagnostics;
  page.on('console', (message) => {
    diagnostics.push(`${message.type()}: ${message.text()}`);
  });
  page.on('pageerror', (error) => {
    diagnostics.push(`pageerror: ${error.stack || error.message}`);
  });
  page.on('requestfailed', (request) => {
    diagnostics.push(`requestfailed ${request.method()} ${request.url()}: ${request.failure()?.errorText}`);
  });
  page.on('response', (response) => {
    if (response.status() >= 400) {
      diagnostics.push(`response ${response.status()} ${response.url()}`);
    }
  });
  await installApiFixtures(page, scenario, target, interaction);
  if (scenario.warmupPath) {
    await gotoStable(page, new URL(scenario.warmupPath, url).toString());
    // A fixed post-navigation delay is not enough under a full serial run: a
    // lazy data-router can still be mounting when the hash is changed, so the
    // first hashchange is lost and the URL/content disagree. Navigate only
    // after the warmup React/Umi root has observably mounted.
    await waitForMountedContent(page, diagnostics);
    if (target === 'oracle' && scenario.seedLegacyAdminStore) {
      await seedLegacyAdminStore(page, scenario);
    }
    await navigateAfterWarmup(page, url);
  } else {
    await gotoStable(page, url);
  }
  if (target === 'oracle' && scenario.seedLegacyAdminStore) {
    await seedLegacyAdminStore(page, scenario);
  }
  const readySelector = interaction.readySelector ?? scenario.readySelector;
  if (readySelector) {
    await waitForReadySelector(page, readySelector, diagnostics);
  }
  if (scenario.postReadyDelay) {
    await page.waitForTimeout(scenario.postReadyDelay);
  }
  await waitForMountedContent(page, diagnostics);
  await waitForFontsBeforeCapture(page, diagnostics);
  await waitForFixedColumnLayout(page);
}

// Port of runInteractionTarget: one world, isolated context. Asserts the raw
// result is useful, then returns the Tier-1-reduced, CJK-collapsed result.
async function runOneWorld(browser, url, scenario, interaction, viewport, target) {
  const context = await browser.newContext({
    viewport: { width: viewport.width, height: viewport.height },
    ...(interaction.userAgent ? { userAgent: interaction.userAgent } : {}),
  });
  const page = await context.newPage();
  try {
    await preparePageForInteraction(page, url, scenario, target, interaction);
    const result = await interaction.run(page);
    assertUsefulInteraction(interaction.label, result, target);
    return collapseCjkDeep(normalizeInteractionResult(interaction.label, result));
  } catch (error) {
    const snapshot = await readDebugSnapshot(page).catch(() => ({
      body: 'unavailable',
      title: 'unavailable',
      url: page.url(),
    }));
    throw new Error(
      `${interaction.label}/${viewport.label}/${target}: ${error.message}\n` +
        `URL: ${snapshot.url}\nTitle: ${snapshot.title}\nBody: ${snapshot.body}\n` +
        `Diagnostics: ${(page.__visualParityDiagnostics ?? []).slice(-40).join(' | ')}`,
    );
  } finally {
    await context.close();
  }
}

// Run one interaction against both worlds and assert cross-world Tier-1 equality.
// Mirrors the per-item body of runInteractionParity.
export async function runParityScenario({ browser, interaction, projectName }) {
  const viewport = viewportByLabel(projectName);
  const scenario = getScenario(interaction.scenarioLabel);

  const sourceResult = await runOneWorld(
    browser,
    new URL(scenario.path, sourceBaseUrl).toString(),
    scenario,
    interaction,
    viewport,
    'source',
  );

  // Accessibility is a quality gate for the redesigned source. The frozen
  // legacy oracle is intentionally not scanned: it is neither shipped nor a
  // valid baseline for modern shadcn semantics.
  if (interaction.sourceOnly) {
    return { sourceResult, oracleResult: null };
  }

  await delay(250);

  const oracleResult = await runOneWorld(
    browser,
    new URL(scenario.path, oracleBaseUrl()).toString(),
    scenario,
    interaction,
    viewport,
    'oracle',
  );

  if (stableJson(sourceResult) !== stableJson(oracleResult)) {
    throw new Error(
      `Interaction parity mismatch for ${interaction.label}/${viewport.label}:\n` +
        `source: ${JSON.stringify(sourceResult)}\noracle: ${JSON.stringify(oracleResult)}`,
    );
  }

  return { sourceResult, oracleResult };
}
