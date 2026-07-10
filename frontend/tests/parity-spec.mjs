// Spec factory: one call per surface generates that group's parity tests. All
// spec files are thin wrappers around this so the run/skip/compare shape stays
// identical across surfaces. The desktop/mobile split is handled by Playwright
// projects; each test skips itself on a viewport its interaction opts out of.
import { test } from '@playwright/test';
import { interactions } from './lib/interaction-scenarios.mjs';
import { groupOf } from './lib/spec-groups.mjs';
import { interactionAppliesToViewport, runParityScenario } from './lib/world.mjs';

export function defineParitySpec(groupName) {
  const groupInteractions = interactions.filter((interaction) => groupOf(interaction) === groupName);
  if (!groupInteractions.length) {
    throw new Error(`No interactions found for parity spec group "${groupName}"`);
  }

  test.describe(`parity: ${groupName}`, () => {
    for (const interaction of groupInteractions) {
      test(interaction.label, async ({ browser }, testInfo) => {
        test.skip(
          !interactionAppliesToViewport(interaction, testInfo.project.name),
          `interaction "${interaction.label}" does not run on ${testInfo.project.name}`,
        );
        await runParityScenario({ browser, interaction, projectName: testInfo.project.name });
      });
    }
  });
}
