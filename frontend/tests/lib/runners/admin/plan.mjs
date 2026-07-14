import {
  adminPlanDrawerState,
  adminMutationFailureState,
  openAdminPlanRowEditor,
  deleteAdminRowWithConfirm,
  clickAdminOrderRowAction,
} from '../../state-readers/admin.mjs';
import {
  clickFirstVisible,
  waitForVisibleText,
  fillVisibleAt,
  clickVisibleAt,
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
  waitForVisibleElementsHidden,
  waitForPagePropertyAtLeast,
  selectLegacyFormOption,
  legacySelectDropdownState,
  openLegacySelectByLabel,
  focusFirstVisible,
  keyboardFocusState,
} from '../../dom-helpers.mjs';
import { hoverTooltipInteraction } from '../../tooltip-helpers.mjs';
import { clonePageRequests } from '../../json-util.mjs';
import {
  adminPlanCreateSelector,
  adminDrawerOpenSelector,
  adminDrawerTitleSelector,
  adminDrawerInputGroupControlSelector,
  adminDrawerInputSelector,
  adminDrawerSelectTriggerSelector,
  adminSelectOptionSelector,
  adminSelectDropdownSelector,
  adminPlanForceUpdateSelector,
  adminPlanSubmitSelector,
  adminTableSwitchSelector,
  adminTableRowSelector,
} from '../../selectors.mjs';

// Price and quota fields use shadcn InputGroupInput while the frozen antd form
// exposes ordinary inputs. A selector list preserves document order in both
// worlds, so the existing contract field indexes remain stable.
const adminPlanInputSelector =
  `${adminDrawerInputSelector}, ${adminDrawerInputGroupControlSelector}`;

export async function runAdminPlanCreateDrawerInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  const before = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanCreateSelector);
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建订阅');
  await fillVisibleAt(page, adminPlanInputSelector, 0, 'Parity Plan');
  await fillVisibleAt(page, adminPlanInputSelector, 1, '<p>Parity plan body</p>');
  await fillVisibleAt(page, adminPlanInputSelector, 2, '12.34');
  await fillVisibleAt(page, adminPlanInputSelector, 3, '23.45');
  await fillVisibleAt(page, adminPlanInputSelector, 8, '199.00');
  await fillVisibleAt(page, adminPlanInputSelector, 10, '250');
  await fillVisibleAt(page, adminPlanInputSelector, 11, '7');
  await fillVisibleAt(page, adminPlanInputSelector, 12, '99');
  await fillVisibleAt(page, adminPlanInputSelector, 13, '50');
  await clickVisibleAt(page, adminDrawerSelectTriggerSelector, 0);
  await waitForVisibleText(page, adminSelectOptionSelector, 'Default');
  const groupDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['Default']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await clickVisibleAt(page, adminDrawerSelectTriggerSelector, 1);
  await waitForVisibleText(page, adminSelectOptionSelector, '按月重置');
  const resetDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['按月重置']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await clickFirstVisible(page, adminPlanForceUpdateSelector);
  await page.waitForTimeout(100);
  const filled = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  await waitForVisibleElementsHidden(page, adminDrawerTitleSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPlanFetchCount',
    initialPlanFetchCount + 1,
  );
  const closed = await adminPlanDrawerState(page);
  return {
    before,
    closed,
    filled,
    groupDropdown,
    planFetchDelta: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
    resetDropdown,
    saveRequests: (page.__visualParityAdminPlanSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminPlanSaveFailureInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  const before = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanCreateSelector);
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建订阅');
  await fillVisibleAt(page, adminPlanInputSelector, 0, 'Parity Failed Plan');
  await fillVisibleAt(page, adminPlanInputSelector, 1, '<p>Plan failure body</p>');
  await fillVisibleAt(page, adminPlanInputSelector, 2, '12.34');
  await fillVisibleAt(page, adminPlanInputSelector, 10, '250');
  await selectLegacyFormOption(page, adminDrawerOpenSelector, '权限组', ['Default']);
  await page.waitForTimeout(100);
  const filled = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanSaveCount', 1);
  await page.waitForTimeout(350);
  const after = await adminPlanDrawerState(page);
  return {
    after,
    before,
    filled,
    planFetchDelta: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminPlanSaveRequests),
  };
}

export async function runAdminPlanCreateGroupSelectDropdownInteraction(page) {
  await clickFirstVisible(page, adminPlanCreateSelector);
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建订阅');
  const before = await legacySelectDropdownState(page, adminDrawerOpenSelector);
  await clickVisibleAt(page, adminDrawerSelectTriggerSelector, 0);
  await waitForVisibleText(page, adminSelectOptionSelector, 'Default');
  await page.waitForTimeout(700);
  const opened = await legacySelectDropdownState(page, adminDrawerOpenSelector);
  return { before, opened };
}

export async function runAdminPlanResetMethodMatrixInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  await clickFirstVisible(page, adminPlanCreateSelector);
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建订阅');
  await fillVisibleAt(page, adminPlanInputSelector, 0, 'Parity Reset Matrix');
  await fillVisibleAt(page, adminPlanInputSelector, 1, '<p>Reset method matrix</p>');
  await fillVisibleAt(page, adminPlanInputSelector, 2, '10.00');
  await fillVisibleAt(page, adminPlanInputSelector, 9, '2.00');
  await fillVisibleAt(page, adminPlanInputSelector, 10, '128');
  await selectLegacyFormOption(page, adminDrawerOpenSelector, '权限组', ['Default']);
  await openLegacySelectByLabel(page, adminDrawerOpenSelector, '流量重置方式');
  await waitForVisibleText(page, adminSelectOptionSelector, '每年1月1日');
  const resetDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['每月1号']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  const monthlyFirst = await adminPlanDrawerState(page);
  await selectLegacyFormOption(page, adminDrawerOpenSelector, '流量重置方式', ['不重置']);
  const neverReset = await adminPlanDrawerState(page);
  await selectLegacyFormOption(page, adminDrawerOpenSelector, '流量重置方式', ['每月1号']);
  const final = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  await waitForVisibleElementsHidden(page, adminDrawerTitleSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPlanFetchCount',
    initialPlanFetchCount + 1,
  );
  const closed = await adminPlanDrawerState(page);
  return {
    closed,
    final,
    monthlyFirst,
    neverReset,
    planFetchDelta: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
    resetDropdown,
    saveRequests: (page.__visualParityAdminPlanSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminPlanDrawerKeyboardCloseInteraction(page) {
  const before = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanCreateSelector);
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建订阅');
  const opened = await adminPlanDrawerState(page);
  await focusFirstVisible(page, adminDrawerOpenSelector);
  const focused = await keyboardFocusState(page);
  await page.keyboard.press('Escape');
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  await waitForVisibleElementsHidden(page, adminDrawerTitleSelector);
  const closed = await adminPlanDrawerState(page);
  return { before, closed, focused, opened };
}

export async function runAdminPlanEditDrawerInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  const before = await adminPlanDrawerState(page);
  await openAdminPlanRowEditor(page, 'Pro');
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑订阅');
  await page.waitForFunction(
    (inputSelector) =>
      Array.from(document.querySelectorAll(inputSelector)).some(
        (element) => 'value' in element && element.value === 'Pro',
      ),
    adminPlanInputSelector,
    { timeout: 5_000 },
  );
  const opened = await adminPlanDrawerState(page);
  await fillVisibleAt(page, adminPlanInputSelector, 0, 'Parity Edited Plan');
  await fillVisibleAt(page, adminPlanInputSelector, 1, '<p>Edited plan body</p>');
  await fillVisibleAt(page, adminPlanInputSelector, 2, '88.88');
  await fillVisibleAt(page, adminPlanInputSelector, 10, '300');
  await fillVisibleAt(page, adminPlanInputSelector, 11, '8');
  await clickVisibleAt(page, adminDrawerSelectTriggerSelector, 1);
  await waitForVisibleText(page, adminSelectOptionSelector, '不重置');
  const resetDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['不重置']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await clickFirstVisible(page, adminPlanForceUpdateSelector);
  await page.waitForTimeout(100);
  const edited = await adminPlanDrawerState(page);
  await clickFirstVisible(page, adminPlanSubmitSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  await waitForVisibleElementsHidden(page, adminDrawerTitleSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPlanFetchCount',
    initialPlanFetchCount + 1,
  );
  const closed = await adminPlanDrawerState(page);
  return {
    before,
    closed,
    edited,
    opened,
    planFetchDelta: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
    resetDropdown,
    saveRequests: (page.__visualParityAdminPlanSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminPlanRenewTooltipInteraction(page) {
  return hoverTooltipInteraction(page, [
    'thead [data-slot="header-tooltip-trigger"]',
    '.ant-table-thead .anticon-question-circle',
  ]);
}

export async function runAdminMutationFailureMatrixInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  const beforePlan = await adminMutationFailureState(page);
  await clickVisibleAt(page, adminTableSwitchSelector, 0);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanUpdateCount', 1);
  await page.waitForTimeout(350);
  const planSwitchFailed = await adminMutationFailureState(page);

  const planDeleteDropdown = await adminMutationFailureState(page);
  await deleteAdminRowWithConfirm(page, 'Pro', 'plan-delete-', async () => {
    await clickAdminOrderRowAction(page, 'Pro', '操作');
    await waitForVisibleText(page, '.ant-dropdown-menu-item', '删除');
    await clickFirstVisibleTextStable(page, '.ant-dropdown-menu-item', ['删除']);
  });
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanDropCount', 1);
  await page.waitForTimeout(350);
  const planDeleteFailed = await adminMutationFailureState(page);

  const initialNoticeFetchCount = page.__visualParityAdminNoticeFetchCount ?? 0;
  await page.evaluate(() => {
    window.location.hash = '/notice';
  });
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminNoticeFetchCount',
    initialNoticeFetchCount + 1,
  );
  await page.waitForSelector(adminTableRowSelector, { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(150);
  const beforeNotice = await adminMutationFailureState(page);
  await clickVisibleAt(page, adminTableSwitchSelector, 0);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeShowCount', 1);
  await page.waitForTimeout(350);
  const noticeSwitchFailed = await adminMutationFailureState(page);
  await deleteAdminRowWithConfirm(page, 'Notice A', 'notice-delete-', async () => {
    await clickFirstVisibleText(page, '.ant-table-tbody a', ['删除']);
  });
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeDropCount', 1);
  await page.waitForTimeout(350);
  const noticeDeleteFailed = await adminMutationFailureState(page);

  const initialServerFetchCount = page.__visualParityAdminServerNodeFetchCount ?? 0;
  await page.evaluate(() => {
    window.location.hash = '/server/manage';
  });
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerNodeFetchCount',
    initialServerFetchCount + 1,
  );
  await waitForVisibleText(page, 'button, .ant-btn', '编辑排序');
  const beforeServerSort = await adminMutationFailureState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', ['编辑排序']);
  await waitForVisibleText(page, 'button, .ant-btn', '保存排序');
  await page.waitForTimeout(150);
  const serverSortMode = await adminMutationFailureState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', ['保存排序']);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerSortCount', 1);
  await page.waitForTimeout(350);
  const serverSortFailed = await adminMutationFailureState(page);

  return {
    beforeNotice,
    beforePlan,
    beforeServerSort,
    fetchDeltas: {
      notice: (page.__visualParityAdminNoticeFetchCount ?? 0) - initialNoticeFetchCount,
      plan: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
      server: (page.__visualParityAdminServerNodeFetchCount ?? 0) - initialServerFetchCount,
    },
    noticeDeleteFailed,
    noticeDropRequests: clonePageRequests(page.__visualParityAdminNoticeDropRequests),
    noticeShowRequests: clonePageRequests(page.__visualParityAdminNoticeShowRequests),
    noticeSwitchFailed,
    planDeleteDropdown,
    planDeleteFailed,
    planDropRequests: clonePageRequests(page.__visualParityAdminPlanDropRequests),
    planSwitchFailed,
    planUpdateRequests: clonePageRequests(page.__visualParityAdminPlanUpdateRequests),
    serverSortFailed,
    serverSortMode,
    serverSortRequests: clonePageRequests(page.__visualParityAdminServerSortRequests),
  };
}
