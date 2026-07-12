import {
  activeTabState,
  clickVisibleAt,
  fillVisibleAt,
  blurVisibleAt,
  waitForPagePropertyAtLeast,
} from '../../dom-helpers.mjs';
import { adminConfigSaveFailureState } from '../../state-readers/admin.mjs';
import { adminConfigTabSelector, adminConfigFieldInputSelector } from '../../selectors.mjs';
import { clonePageRequests } from '../../json-util.mjs';

export async function runAdminConfigTabsInteraction(page) {
  const before = await activeTabState(page);
  await clickVisibleAt(page, adminConfigTabSelector, 1);
  await page.waitForTimeout(250);
  const second = await activeTabState(page);
  await clickVisibleAt(page, adminConfigTabSelector, 2);
  await page.waitForTimeout(250);
  const third = await activeTabState(page);
  return { before, second, third };
}

export async function runAdminConfigSaveFailureMatrixInteraction(page) {
  const initialConfigFetchCount = page.__visualParityAdminConfigFetchCount ?? 0;
  const before = await adminConfigSaveFailureState(page);
  await fillVisibleAt(page, adminConfigFieldInputSelector, 0, 'Parity Config Failure');
  // The redesigned config field commits on blur (onChange only stages a draft);
  // the legacy field already saved on the fill's input/change event, so an
  // explicit blur triggers the source save without adding a second legacy save.
  await blurVisibleAt(page, adminConfigFieldInputSelector, 0);
  await page.waitForTimeout(150);
  const edited = await adminConfigSaveFailureState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminConfigSaveCount', 1, 7_000);
  await page.waitForTimeout(350);
  const configFailed = await adminConfigSaveFailureState(page);

  return {
    before,
    configFailed,
    configFetchDelta: (page.__visualParityAdminConfigFetchCount ?? 0) - initialConfigFetchCount,
    configSaveRequests: clonePageRequests(page.__visualParityAdminConfigSaveRequests),
    edited,
  };
}
