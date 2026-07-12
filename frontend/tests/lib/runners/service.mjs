import {
  setServiceTableScrollLeft,
  serviceTableScrollState,
} from '../state-readers/service.mjs';
import {
  hoverAllTooltipTargetsInteraction,
  hoverTooltipInteraction,
} from '../tooltip-helpers.mjs';

export async function runNodeTableScrollInteraction(page) {
  const before = await serviceTableScrollState(page);
  await setServiceTableScrollLeft(page, 'right');
  await page.waitForTimeout(150);
  const afterRight = await serviceTableScrollState(page);
  await setServiceTableScrollLeft(page, 'middle');
  await page.waitForTimeout(150);
  const afterMiddle = await serviceTableScrollState(page);

  return { afterMiddle, afterRight, before };
}

export async function runUserNodeTooltipsInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, [
    '[data-testid="node-table"] [data-slot="header-tooltip-trigger"]',
    '.ant-table-thead .anticon-question-circle',
  ]);
}

export async function runTrafficTableScrollInteraction(page) {
  const before = await serviceTableScrollState(page);
  await setServiceTableScrollLeft(page, 'right');
  await page.waitForTimeout(150);
  const afterRight = await serviceTableScrollState(page);
  await setServiceTableScrollLeft(page, 'middle');
  await page.waitForTimeout(150);
  const afterMiddle = await serviceTableScrollState(page);

  return { afterMiddle, afterRight, before };
}

export async function runUserTrafficTotalTooltipInteraction(page) {
  await setServiceTableScrollLeft(page, 'right');
  await page.waitForTimeout(150);
  return hoverTooltipInteraction(page, [
    '[data-testid="traffic-table"] [data-slot="header-tooltip-trigger"]',
    '.ant-table-fixed .anticon-question-circle',
    '.ant-table-thead .anticon-question-circle',
  ]);
}
