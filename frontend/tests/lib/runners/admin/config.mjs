import {
  activeTabState,
  clickVisibleAt,
  fillVisibleAt,
  blurVisibleAt,
  waitForPagePropertyAtLeast,
  waitForVisibleText,
  clickFirstVisibleText,
  waitForVisibleElementsHidden,
} from '../../dom-helpers.mjs';
import {
  adminConfigSaveFailureState,
  adminThemeSaveFailureState,
  adminThemeModalState,
} from '../../state-readers/admin.mjs';
import {
  adminConfigTabSelector,
  adminConfigFieldInputSelector,
  adminThemeCardTitleSelector,
  adminOverlayOpenSelector,
  adminDrawerTitleSelector,
  adminDrawerInputSelector,
  adminDrawerFooterButtonSelector,
} from '../../selectors.mjs';
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
  const initialThemeFetchCount = page.__visualParityAdminThemeFetchCount ?? 0;
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

  await page.evaluate(() => {
    window.location.hash = '/config/theme';
  });
  await page.waitForSelector(adminThemeCardTitleSelector, { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(500);
  const themeBefore = await adminThemeSaveFailureState(page);
  await clickFirstVisibleText(page, 'button', ['主题设置']);
  await page.waitForSelector(adminOverlayOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '配置默认主题主题');
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Theme Failure');
  await page.waitForTimeout(100);
  const themeFilled = await adminThemeSaveFailureState(page);
  await clickVisibleAt(page, adminDrawerFooterButtonSelector, 1);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminThemeSaveCount', 1, 5_000);
  await page.waitForTimeout(350);
  const themeFailed = await adminThemeSaveFailureState(page);

  return {
    before,
    configFailed,
    configFetchDelta: (page.__visualParityAdminConfigFetchCount ?? 0) - initialConfigFetchCount,
    configSaveRequests: clonePageRequests(page.__visualParityAdminConfigSaveRequests),
    edited,
    themeBefore,
    themeFailed,
    themeFetchDelta: (page.__visualParityAdminThemeFetchCount ?? 0) - initialThemeFetchCount,
    themeFilled,
    themeSaveRequests: clonePageRequests(page.__visualParityAdminThemeSaveRequests),
  };
}

export async function runAdminThemeSettingsInteraction(page) {
  await clickFirstVisibleText(page, 'button', ['主题设置']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Theme Title');
  await page.waitForTimeout(100);
  const opened = await adminThemeModalState(page);
  await clickVisibleAt(page, adminDrawerFooterButtonSelector, 0);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  const closed = await adminThemeModalState(page);
  return { closed, opened };
}
