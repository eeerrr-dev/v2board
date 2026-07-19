import {
  activeTabState,
  clickVisibleAt,
  clickFirstVisibleTextStable,
  fillVisibleAt,
  blurVisibleAt,
  visibleCount,
  waitForPageProperty,
  waitForPagePropertyAtLeast,
  waitForVisibleElementsHidden,
  waitForVisibleText,
} from '../../dom-helpers.mjs';
import { adminConfigSaveFailureState } from '../../state-readers/admin.mjs';
import {
  adminConfigTabSelector,
  adminConfigFieldInputSelector,
  adminSelectDropdownSelector,
  adminSelectOptionSelector,
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

export async function runAdminConfigUnchangedBlurInteraction(page) {
  const initialSaveCount = page.__visualParityAdminConfigSaveCount ?? 0;
  await clickVisibleAt(page, adminConfigFieldInputSelector, 0);
  await blurVisibleAt(page, adminConfigFieldInputSelector, 0);
  await page.waitForTimeout(350);
  return {
    configSaveDelta: (page.__visualParityAdminConfigSaveCount ?? 0) - initialSaveCount,
  };
}

// §6.11 (native-only): drive the /audit trail's three §7 filter controls and
// capture the canonical GET system/audit-logs query each one mints. Source-only
// — the frozen oracle has no audit surface — so plain data-testid selectors are
// enough; the shared union selectors cover the portaled Radix Select chrome.
export async function runAdminAuditFiltersInteraction(page) {
  await waitForPageProperty(page, '__visualParityLastAdminAuditFetchQuery');
  const initial = {
    query: page.__visualParityLastAdminAuditFetchQuery,
    rowCount: await visibleCount(
      page,
      '[data-testid="audit-table"] [data-slot="table-body"] [data-slot="table-row"]',
    ),
  };

  const applyFilter = async (act) => {
    page.__visualParityLastAdminAuditFetchQuery = null;
    await act();
    await waitForPageProperty(page, '__visualParityLastAdminAuditFetchQuery');
    return { query: page.__visualParityLastAdminAuditFetchQuery };
  };
  const selectOption = async (triggerTestId, optionText) => {
    await page.click(`[data-testid="${triggerTestId}"]`);
    await waitForVisibleText(page, adminSelectOptionSelector, optionText);
    await clickFirstVisibleTextStable(page, adminSelectOptionSelector, [optionText]);
    await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  };

  const surfaceFiltered = await applyFilter(() => selectOption('audit-surface-filter', '员工'));
  const methodFiltered = await applyFilter(() => selectOption('audit-method-filter', 'POST'));
  // The email input drafts locally and mints its like clause on Enter.
  const emailFiltered = await applyFilter(async () => {
    await page.locator('[data-testid="audit-email-filter"]').fill('staff@example.com');
    await page.keyboard.press('Enter');
  });

  return { emailFiltered, initial, methodFiltered, surfaceFiltered };
}
