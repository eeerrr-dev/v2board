import {
  activeTabState,
  clickVisibleAt,
  clickFirstVisibleTextStable,
  visibleCount,
  waitForPageProperty,
  waitForPagePropertyAtLeast,
  waitForVisibleElementsHidden,
  waitForVisibleText,
} from '../../dom-helpers.mjs';
import { adminConfigSaveFailureState } from '../../state-readers/admin.mjs';
import {
  adminConfigTabSelector,
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
  await page.locator('[data-testid="config-app_name"]').fill('Parity Config Failure');
  // A section is one explicit transaction: editing only stages a draft. The
  // PATCH must begin on Save, never on input/change/blur.
  await page.locator('[data-testid="config-app_name"]').blur();
  const edited = await adminConfigSaveFailureState(page);
  const configSaveCountBeforeSubmit = page.__visualParityAdminConfigSaveCount ?? 0;
  await page.locator('[data-testid="config-save"]').click();
  await waitForPagePropertyAtLeast(page, '__visualParityAdminConfigSaveCount', 1, 7_000);
  await page.waitForTimeout(350);
  const configFailed = await adminConfigSaveFailureState(page);
  const errorText = await page.locator('[data-testid="config-save-error"]').textContent();

  return {
    before,
    configFailed,
    configFetchDelta: (page.__visualParityAdminConfigFetchCount ?? 0) - initialConfigFetchCount,
    configSaveCountBeforeSubmit,
    configSaveRequests: clonePageRequests(page.__visualParityAdminConfigSaveRequests),
    edited,
    errorText,
  };
}

export async function runAdminConfigDraftDiscardInteraction(page) {
  const initialSaveCount = page.__visualParityAdminConfigSaveCount ?? 0;
  const readState = () =>
    page.evaluate(() => {
      const input = document.querySelector('[data-testid="config-app_name"]');
      const save = document.querySelector('[data-testid="config-save"]');
      const discard = document.querySelector('[data-testid="config-discard"]');
      return {
        value: input instanceof HTMLInputElement ? input.value : null,
        saveDisabled: save instanceof HTMLButtonElement ? save.disabled : null,
        discardDisabled: discard instanceof HTMLButtonElement ? discard.disabled : null,
      };
    });
  const before = await readState();
  await page.locator('[data-testid="config-app_name"]').fill('Parity Config Draft');
  await page.locator('[data-testid="config-app_name"]').blur();
  const staged = await readState();
  await page.locator('[data-testid="config-discard"]').click();
  await page.waitForFunction(() => {
    const input = document.querySelector('[data-testid="config-app_name"]');
    return input instanceof HTMLInputElement && input.value !== 'Parity Config Draft';
  });
  const discarded = await readState();
  return {
    before,
    configSaveDelta: (page.__visualParityAdminConfigSaveCount ?? 0) - initialSaveCount,
    discarded,
    staged,
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
