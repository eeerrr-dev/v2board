import { fillFirstVisibleIfPresent, clickFirstVisible } from '../dom-helpers.mjs';
import { knowledgeSearchInputSelector } from '../fixture-data.mjs';
import { knowledgeState } from '../state-readers/knowledge.mjs';

export async function runKnowledgeDrawerInteraction(page) {
  await fillFirstVisibleIfPresent(page, knowledgeSearchInputSelector, 'router');
  await page.waitForTimeout(350);
  const before = await knowledgeState(page);
  await clickFirstVisible(page, '[data-testid="knowledge-item"], .list-group-item');
  await page.waitForSelector('[data-testid="knowledge-sheet-title"], .ant-drawer-open .ant-drawer-title', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      Array.from(
        document.querySelectorAll('[data-testid="knowledge-sheet-title"], .ant-drawer-title'),
      ).some((element) => element.textContent?.includes('Copy Article')),
    null,
    { timeout: 5_000 },
  );
  const opened = await knowledgeState(page);
  await clickFirstVisible(page, '[data-testid="knowledge-sheet"] button, .ant-drawer-close');
  await page.waitForFunction(
    () =>
      !document.querySelector('[data-testid="knowledge-sheet"]') &&
      !document.querySelector('.ant-drawer-open'),
    null,
    { timeout: 5_000 },
  );
  const closed = await knowledgeState(page);
  return { before, closed, opened };
}

export async function runUserKnowledgeExtremeContentMatrixInteraction(page) {
  await fillFirstVisibleIfPresent(page, knowledgeSearchInputSelector, 'extreme legacy');
  await page.waitForTimeout(350);
  const filtered = await knowledgeState(page);
  await clickFirstVisible(page, '[data-testid="knowledge-item"], .list-group-item');
  await page.waitForSelector('[data-testid="knowledge-sheet-title"], .ant-drawer-open .ant-drawer-title', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      Array.from(
        document.querySelectorAll('[data-testid="knowledge-sheet-title"], .ant-drawer-title'),
      ).some((element) => element.textContent?.includes('Extreme Legacy')),
    null,
    { timeout: 5_000 },
  );
  const opened = await knowledgeState(page);
  return { filtered, opened };
}
