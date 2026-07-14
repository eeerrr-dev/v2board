import {
  addAdminUserFilterCondition,
  adminOrderAssignModalState,
  adminTablePaginationState,
  adminUserAssignActionState,
  adminUserBulkActionState,
  adminUserConfirmState,
  adminUserCopyActionState,
  adminUserCreateModalState,
  adminUserDestructiveFailureState,
  adminUserEditActionState,
  adminUserExportDownloadState,
  adminUserFilterDateFieldState,
  adminUserInviteActionState,
  adminUserOrdersActionState,
  adminUserSendMailModalState,
  adminUserSortState,
  adminUserTrafficActionState,
  adminUsersExtremeViewportState,
  applyAdminUserEmailFilter,
  clickAdminUserCreateSubmit,
  clickAdminUserManageSubmit,
  clickAdminUserPage,
  clickAdminUserSendMailCancel,
  clickAdminUserSendMailSubmit,
  fillAdminOverlayInput,
  fillAdminUserCreatePassword,
  fillAdminUserSendMailContent,
  fillAdminUserSendMailSubject,
  installClipboardProbe,
  installDownloadProbe,
  openAdminUserCreateDialog,
  openAdminUserFilterFieldSelect,
  openAdminUserFilterSheet,
  openAdminUserPageSizeChanger,
  openAdminUserRowActionMenu,
  openAdminUserToolbarDropdown,
  selectAdminOverlayOption,
  waitForOverlayInputValue,
} from '../../state-readers/admin.mjs';
import {
  clickFirstVisible,
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
  clickVisibleAt,
  fillFirstVisible,
  fillVisibleAt,
  firstInputValue,
  legacySelectDropdownState,
  visibleTexts,
  waitForPageProperty,
  waitForPagePropertyAtLeast,
  waitForVisibleElementsHidden,
  waitForVisibleText,
} from '../../dom-helpers.mjs';
import {
  adminConfirmButtonsSelector,
  adminConfirmContentSelector,
  adminConfirmDialogSelector,
  adminConfirmPrimarySelector,
  adminConfirmTitleSelector,
  adminDialogOpenSelector,
  adminDrawerOpenSelector,
  adminDrawerSelectTriggerSelector,
  adminDrawerTitleSelector,
  adminMenuItemSelector,
  adminSelectDropdownSelector,
  adminSelectOptionSelector,
  adminTableRowSelector,
} from '../../selectors.mjs';
import { clonePageRequests } from '../../json-util.mjs';

export async function runAdminUsersFilterInteraction(page) {
  await openAdminUserFilterSheet(page);
  await addAdminUserFilterCondition(page);
  const shadcnSheet = await page.$('[data-testid="user-filter-sheet"]');
  if (shadcnSheet) {
    await page.locator('[data-testid="user-filter-value-0"]').fill('visual@example.com');
    await page.waitForTimeout(100);
    return {
      firstInput: await firstInputValue(page, '[data-testid="user-filter-value-0"]'),
      visibleButtons: await visibleTexts(page, '[data-testid="user-filter-sheet"] button', 6),
    };
  }
  await fillFirstVisible(page, '.v2board-filter-drawer .ant-input', 'visual@example.com');
  await page.waitForTimeout(100);
  return {
    firstInput: await firstInputValue(page, '.v2board-filter-drawer .ant-input'),
    visibleButtons: await visibleTexts(page, '.v2board-filter-drawer .ant-btn', 6),
  };
}

export async function runAdminUsersFilterFieldSelectDropdownInteraction(page) {
  await openAdminUserFilterSheet(page);
  await addAdminUserFilterCondition(page);
  const before = await legacySelectDropdownState(page, adminDrawerOpenSelector);
  await openAdminUserFilterFieldSelect(page);
  await waitForVisibleText(page, adminSelectOptionSelector, '到期时间');
  await page.waitForTimeout(700);
  const opened = await legacySelectDropdownState(page, adminDrawerOpenSelector);
  return { before, opened };
}

export async function runAdminUsersFilterExpiryPickerInteraction(page) {
  await openAdminUserFilterSheet(page);
  await addAdminUserFilterCondition(page);
  const before = await adminUserFilterDateFieldState(page);
  // Switch the condition field to 到期时间 with a real pointer (both antd Select and
  // Radix Select need one). The redesigned value input becomes a native
  // datetime-local input; the antd oracle exposes an `.ant-calendar-picker-input`.
  // The calendar popup chrome is Tier-2 presentation, so both reduce to whether a
  // date filter became reachable.
  await selectAdminOverlayOption(page, 0, '到期时间');
  await page.waitForTimeout(200);
  const opened = await adminUserFilterDateFieldState(page);
  return { before, opened };
}

export async function runAdminUsersPaginationMatrixInteraction(page) {
  await page.waitForSelector(
    '.ant-pagination-item-2, [data-testid="user-page"][data-page="2"]',
    { state: 'visible', timeout: 5_000 },
  );
  const before = await adminTablePaginationState(page, 'user');
  page.__visualParityLastAdminUserFetchQuery = null;
  await clickAdminUserPage(page, 2);
  await waitForPageProperty(page, '__visualParityLastAdminUserFetchQuery');
  await page.waitForTimeout(250);
  const page2 = await adminTablePaginationState(page, 'user');
  if (page2.sizeChangerCount === 0) {
    return { before, page2, pageSize50: null, sizeDropdown: { skipped: 'not-visible' } };
  }
  page.__visualParityLastAdminUserFetchQuery = null;
  await openAdminUserPageSizeChanger(page);
  await waitForVisibleText(page, adminSelectOptionSelector, '50 条/页');
  const sizeDropdown = await legacySelectDropdownState(page, adminSelectDropdownSelector);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['50 条/页']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await waitForPageProperty(page, '__visualParityLastAdminUserFetchQuery');
  await page.waitForTimeout(250);
  const pageSize50 = await adminTablePaginationState(page, 'user');
  return { before, page2, pageSize50, sizeDropdown };
}

export async function runAdminUsersSortMatrixInteraction(page) {
  const before = await adminUserSortState(page);
  page.__visualParityLastAdminUserFetchQuery = null;
  await clickFirstVisibleText(page, '.ant-table-thead th, [data-slot="table-head"]', ['状态']);
  await waitForPageProperty(page, '__visualParityLastAdminUserFetchQuery');
  await page.waitForTimeout(250);
  const asc = await adminUserSortState(page);
  page.__visualParityLastAdminUserFetchQuery = null;
  await clickFirstVisibleText(page, '.ant-table-thead th, [data-slot="table-head"]', ['状态']);
  await waitForPageProperty(page, '__visualParityLastAdminUserFetchQuery');
  await page.waitForTimeout(250);
  const desc = await adminUserSortState(page);
  return { asc, before, desc };
}

export async function runAdminUserBulkBanConfirmInteraction(page) {
  return runAdminUserBulkConfirmInteraction(page, '批量封禁', '确定要进行封禁吗？');
}

export async function runAdminUserBulkDeleteConfirmInteraction(page) {
  return runAdminUserBulkConfirmInteraction(page, '批量删除', '确定要进行删除吗？');
}

export async function runAdminUserBulkConfirmInteraction(page, actionText, contentText) {
  const before = await adminUserBulkActionState(page);
  await applyAdminUserEmailFilter(page);
  const filtered = await adminUserBulkActionState(page);
  await openAdminUserToolbarDropdown(page, actionText);
  const dropdown = await adminUserBulkActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, [actionText]);
  await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminConfirmTitleSelector, '提醒');
  await waitForVisibleText(page, adminConfirmContentSelector, contentText);
  const opened = await adminUserBulkActionState(page);
  await clickVisibleAt(page, adminConfirmButtonsSelector, 0);
  await waitForVisibleElementsHidden(page, adminConfirmDialogSelector);
  const closed = await adminUserBulkActionState(page);
  return { actionText, before, closed, contentText, dropdown, filtered, opened };
}

export async function runAdminUserDestructiveFailureMatrixInteraction(page) {
  const initialFetchCount = page.__visualParityAdminUserFetchCount ?? 0;
  const before = await adminUserDestructiveFailureState(page);
  await openAdminUserRowActionMenu(page, '删除用户');
  const deleteDropdown = await adminUserDestructiveFailureState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['删除用户']);
  await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminConfirmTitleSelector, '删除用户');
  const deleteOpened = await adminUserDestructiveFailureState(page);
  await clickFirstVisible(page, adminConfirmPrimarySelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminUserDeleteCount', 1);
  await waitForVisibleElementsHidden(page, adminConfirmDialogSelector);
  await page.waitForTimeout(350);
  const deleteFailed = await adminUserDestructiveFailureState(page);

  await applyAdminUserEmailFilter(page);
  const filterFetchCount = page.__visualParityAdminUserFetchCount ?? 0;
  const filtered = await adminUserDestructiveFailureState(page);

  await openAdminUserToolbarDropdown(page, '批量封禁');
  const banDropdown = await adminUserDestructiveFailureState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['批量封禁']);
  await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminConfirmTitleSelector, '提醒');
  const banOpened = await adminUserDestructiveFailureState(page);
  await clickFirstVisible(page, adminConfirmPrimarySelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminUserBanCount', 1);
  await waitForVisibleElementsHidden(page, adminConfirmDialogSelector);
  await page.waitForTimeout(350);
  const banFailed = await adminUserDestructiveFailureState(page);

  await openAdminUserToolbarDropdown(page, '批量删除');
  const allDeleteDropdown = await adminUserDestructiveFailureState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['批量删除']);
  await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminConfirmTitleSelector, '提醒');
  const allDeleteOpened = await adminUserDestructiveFailureState(page);
  await clickFirstVisible(page, adminConfirmPrimarySelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminUserAllDeleteCount', 1);
  await waitForVisibleElementsHidden(page, adminConfirmDialogSelector);
  await page.waitForTimeout(350);
  const allDeleteFailed = await adminUserDestructiveFailureState(page);

  return {
    allDeleteDropdown,
    allDeleteFailed,
    allDeleteOpened,
    allDeleteRequests: clonePageRequests(page.__visualParityAdminUserAllDeleteRequests),
    banDropdown,
    banFailed,
    banOpened,
    banRequests: clonePageRequests(page.__visualParityAdminUserBanRequests),
    before,
    deleteDropdown,
    deleteFailed,
    deleteOpened,
    deleteRequests: clonePageRequests(page.__visualParityAdminUserDeleteRequests),
    filtered,
    initialFetchDelta: filterFetchCount - initialFetchCount,
    mutationFetchDelta: (page.__visualParityAdminUserFetchCount ?? 0) - filterFetchCount,
  };
}

export async function runAdminUserExportDownloadMatrixInteraction(page) {
  await installDownloadProbe(page);
  const before = await adminUserExportDownloadState(page);
  await applyAdminUserEmailFilter(page);
  const filtered = await adminUserExportDownloadState(page);
  await openAdminUserToolbarDropdown(page, '导出CSV');
  const dropdown = await adminUserExportDownloadState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['导出CSV']);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminUserDumpCsvCount', 1);
  await page.waitForTimeout(350);
  const downloaded = await adminUserExportDownloadState(page);
  return {
    before,
    downloaded,
    dropdown,
    dumpCsvRequests: clonePageRequests(page.__visualParityAdminUserDumpCsvRequests),
    filtered,
  };
}

export async function runAdminUserCreateModalInteraction(page) {
  const initialGenerateCount = page.__visualParityAdminUserGenerateCount ?? 0;
  const before = await adminUserCreateModalState(page);
  await openAdminUserCreateDialog(page);
  const opened = await adminUserCreateModalState(page);
  await fillAdminOverlayInput(page, 'generate-email-prefix', 0, 'parity.created');
  await fillAdminOverlayInput(page, 'generate-email-suffix', 2, 'example.com');
  await fillAdminUserCreatePassword(page, 'secret123');
  // Open the plan Select (real pointer drives both the antd Select and the Radix
  // trigger), read the option list, then choose Pro.
  await page.locator(adminDrawerSelectTriggerSelector).first().click();
  await waitForVisibleText(page, adminSelectOptionSelector, 'Pro');
  const planDropdown = await adminUserCreateModalState(page);
  await page.locator(adminSelectOptionSelector, { hasText: 'Pro' }).first().click();
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await page.waitForTimeout(100);
  const filled = await adminUserCreateModalState(page);
  await clickAdminUserCreateSubmit(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminUserGenerateCount',
    initialGenerateCount + 1,
  );
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  const closed = await adminUserCreateModalState(page);
  return {
    before,
    closed,
    filled,
    generateRequests: clonePageRequests(page.__visualParityAdminUserGenerateRequests),
    opened,
    planDropdown,
  };
}

export async function runAdminUserCreatePlanSelectDropdownInteraction(page) {
  await openAdminUserCreateDialog(page);
  const before = await legacySelectDropdownState(page, adminDialogOpenSelector);
  await page.locator(adminDrawerSelectTriggerSelector).first().click();
  await waitForVisibleText(page, adminSelectOptionSelector, 'Pro');
  await page.waitForTimeout(300);
  const opened = await legacySelectDropdownState(page, adminDialogOpenSelector);
  return { before, opened };
}

export async function runAdminUserCreateExpiryPickerInteraction(page) {
  await openAdminUserCreateDialog(page);
  // The 到期时间 field is a native date input on the redesigned dialog and an antd
  // calendar-picker input on the oracle. Both reduce to whether a date field is
  // reachable; the calendar popup chrome is Tier-2 presentation.
  const before = await adminUserFilterDateFieldState(page, 'generate-expired');
  const opened = await adminUserFilterDateFieldState(page, 'generate-expired');
  return { before, opened };
}

export async function runAdminUserSendMailModalInteraction(page) {
  const before = await adminUserSendMailModalState(page);
  await openAdminUserToolbarDropdown(page, '发送邮件');
  const dropdown = await adminUserSendMailModalState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['发送邮件']);
  await page.waitForSelector(adminDialogOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '发送邮件');
  const opened = await adminUserSendMailModalState(page);
  await fillAdminUserSendMailSubject(page, 'Parity Mail Subject');
  await fillAdminUserSendMailContent(page, 'Parity mail body\nLine two');
  await page.waitForTimeout(100);
  const filled = await adminUserSendMailModalState(page);
  await clickAdminUserSendMailCancel(page);
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  const closed = await adminUserSendMailModalState(page);
  return { before, closed, dropdown, filled, opened };
}

export async function runAdminUserSendMailSubmitMatrixInteraction(page) {
  const initialSendMailCount = page.__visualParityAdminUserSendMailCount ?? 0;
  const before = await adminUserSendMailModalState(page);

  await openAdminUserToolbarDropdown(page, '发送邮件');
  const successDropdown = await adminUserSendMailModalState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['发送邮件']);
  await page.waitForSelector(adminDialogOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '发送邮件');
  await fillAdminUserSendMailSubject(page, 'Parity Mail Submit Success');
  await fillAdminUserSendMailContent(page, 'Queued success body');
  const successFilled = await adminUserSendMailModalState(page);
  await clickAdminUserSendMailSubmit(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminUserSendMailCount',
    initialSendMailCount + 1,
  );
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  await page.mouse.move(0, 0);
  await page.waitForTimeout(350);
  const successClosed = await adminUserSendMailModalState(page);

  await openAdminUserToolbarDropdown(page, '发送邮件');
  const failureDropdown = await adminUserSendMailModalState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['发送邮件']);
  await page.waitForSelector(adminDialogOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '发送邮件');
  await fillAdminUserSendMailSubject(page, 'Parity Mail Failure');
  await fillAdminUserSendMailContent(page, 'Queued failure body');
  const failureFilled = await adminUserSendMailModalState(page);
  await clickAdminUserSendMailSubmit(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminUserSendMailCount',
    initialSendMailCount + 2,
  );
  await page.waitForTimeout(350);
  const failureKept = await adminUserSendMailModalState(page);

  return {
    before,
    failureDropdown,
    failureFilled,
    failureKept,
    sendMailRequests: clonePageRequests(page.__visualParityAdminUserSendMailRequests),
    successClosed,
    successDropdown,
    successFilled,
  };
}

export async function runAdminUserResetSecretConfirmInteraction(page) {
  const before = await adminUserConfirmState(page);
  await openAdminUserRowActionMenu(page, '重置UUID及订阅URL');
  const dropdown = await adminUserConfirmState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['重置UUID及订阅URL']);
  await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminConfirmTitleSelector, '重置安全信息');
  const opened = await adminUserConfirmState(page);
  await clickVisibleAt(page, adminConfirmButtonsSelector, 0);
  await waitForVisibleElementsHidden(page, adminConfirmDialogSelector);
  const closed = await adminUserConfirmState(page);
  return { before, closed, dropdown, opened };
}

export async function runAdminUserDeleteConfirmInteraction(page) {
  const before = await adminUserConfirmState(page);
  await openAdminUserRowActionMenu(page, '删除用户');
  const dropdown = await adminUserConfirmState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['删除用户']);
  await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminConfirmTitleSelector, '删除用户');
  const opened = await adminUserConfirmState(page);
  await clickVisibleAt(page, adminConfirmButtonsSelector, 0);
  await waitForVisibleElementsHidden(page, adminConfirmDialogSelector);
  const closed = await adminUserConfirmState(page);
  return { before, closed, dropdown, opened };
}

export async function runAdminUserCopyActionInteraction(page) {
  await installClipboardProbe(page);
  const before = await adminUserCopyActionState(page);
  await openAdminUserRowActionMenu(page, '复制订阅URL');
  const dropdown = await adminUserCopyActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['复制订阅URL']);
  // The redesigned surface copies silently through `navigator.clipboard`; the antd
  // oracle copies through `execCommand` + a `复制成功` toast. Wait for whichever
  // observable the copy produced.
  await page.waitForFunction(
    () =>
      (window.__visualParityClipboardWrites ?? []).length > 0 ||
      Boolean(
        document.querySelector(
          '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
        ),
      ),
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(100);
  const copied = await adminUserCopyActionState(page);
  return { before, copied, dropdown };
}

export async function runAdminUserEditActionInteraction(page) {
  const before = await adminUserEditActionState(page);
  await openAdminUserRowActionMenu(page, '编辑');
  const opened = await adminUserEditActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['编辑']);
  await page.waitForSelector(adminDrawerOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '用户管理');
  await waitForOverlayInputValue(page, 'visual-user@example.com');
  const drawer = await adminUserEditActionState(page);
  return { before, drawer, opened };
}

export async function runAdminUserUpdateValidationFailureInteraction(page) {
  const initialUserFetchCount = page.__visualParityAdminUserFetchCount ?? 0;
  const before = await adminUserEditActionState(page);
  await openAdminUserRowActionMenu(page, '编辑');
  const dropdown = await adminUserEditActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['编辑']);
  await page.waitForSelector(adminDrawerOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '用户管理');
  await waitForOverlayInputValue(page, 'visual-user@example.com');
  await fillAdminOverlayInput(page, 'user-drawer-email', 0, 'invalid-email');
  await page.waitForTimeout(100);
  const edited = await adminUserEditActionState(page);
  await clickAdminUserManageSubmit(page);
  // The redesigned RHF/Zod form rejects the malformed email locally; the
  // frozen oracle still submits it and receives the backend validation error.
  // Both outcomes must preserve the drawer and avoid a list refetch.
  await page.waitForTimeout(350);
  const failed = await adminUserEditActionState(page);
  return {
    before,
    dropdown,
    edited,
    failed,
    updateRequests: clonePageRequests(page.__visualParityAdminUserUpdateRequests),
    userFetchDelta: (page.__visualParityAdminUserFetchCount ?? 0) - initialUserFetchCount,
  };
}

export async function runAdminUserAssignActionInteraction(page) {
  const before = await adminUserAssignActionState(page);
  await openAdminUserRowActionMenu(page, '分配订单');
  const opened = await adminUserAssignActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['分配订单']);
  await page.waitForSelector(adminDialogOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '订单分配');
  const modalOpened = await adminOrderAssignModalState(page);
  await selectAdminOverlayOption(page, 0, 'Pro');
  await selectAdminOverlayOption(page, 1, '月付');
  const shadcnAmount = await page.$('[data-testid="assign-amount"]');
  if (shadcnAmount) {
    await page.locator('[data-testid="assign-amount"]').fill('23.45');
  } else {
    await fillVisibleAt(page, '.ant-modal input', 1, '23.45');
  }
  await page.waitForTimeout(100);
  const filled = await adminOrderAssignModalState(page);
  const shadcnSubmit = await page.$('[data-testid="assign-submit"]');
  if (shadcnSubmit) {
    await page.click('[data-testid="assign-submit"]');
  } else {
    await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 1);
  }
  await waitForVisibleElementsHidden(page, adminDialogOpenSelector);
  const closed = await adminOrderAssignModalState(page);
  return {
    assignRequest: page.__visualParityLastAdminOrderAssign ?? null,
    before,
    closed,
    filled,
    modalOpened,
    opened,
  };
}

export async function runAdminUserOrdersActionInteraction(page) {
  const before = await adminUserOrdersActionState(page);
  await openAdminUserRowActionMenu(page, 'TA的订单');
  const opened = await adminUserOrdersActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['TA的订单']);
  await page.waitForFunction(() => window.location.hash.includes('/order'), null, {
    timeout: 5_000,
  });
  await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
  await page.waitForSelector(adminTableRowSelector, { state: 'visible', timeout: 5_000 });
  const navigated = await adminUserOrdersActionState(page);
  return { before, navigated, opened };
}

export async function runAdminUserInviteActionInteraction(page) {
  const before = await adminUserInviteActionState(page);
  await openAdminUserRowActionMenu(page, 'TA的邀请');
  const opened = await adminUserInviteActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['TA的邀请']);
  await waitForPageProperty(page, '__visualParityLastAdminFilteredUserFetchQuery');
  const filtered = await adminUserInviteActionState(page);
  return { before, filtered, opened };
}

export async function runAdminUserTrafficActionInteraction(page) {
  const before = await adminUserTrafficActionState(page);
  await openAdminUserRowActionMenu(page, 'TA的流量记录');
  const opened = await adminUserTrafficActionState(page);
  await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['TA的流量记录']);
  await waitForPageProperty(page, '__visualParityLastAdminUserTrafficQuery');
  await page.waitForSelector(adminDialogOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '流量记录');
  const modal = await adminUserTrafficActionState(page);
  return { before, modal, opened };
}

export async function runAdminUsersExtremeViewportMatrixInteraction(page) {
  const before = await adminUsersExtremeViewportState(page);
  await page.setViewportSize({ width: 320, height: 740 });
  await page.waitForTimeout(600);
  const narrowed = await adminUsersExtremeViewportState(page);
  await openAdminUserFilterSheet(page);
  await page.waitForTimeout(150);
  const filterDrawer = await adminUsersExtremeViewportState(page);
  return { before, filterDrawer, narrowed };
}
