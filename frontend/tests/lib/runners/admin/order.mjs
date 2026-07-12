import {
  adminOrderDetailModalState,
  adminOrderAssignModalState,
  adminOrderStatusDropdownState,
  adminOrderCommissionDropdownState,
  adminOrderFilterPaginationState,
  clickAdminOrderRowAction,
  filterDrawerDebugState,
  selectAdminOverlayOption,
  openAdminOrderRowTrigger,
} from '../../state-readers/admin.mjs';
import {
  clickFirstVisible,
  clickFirstVisibleText,
  clickFirstVisibleTextInViewport,
  clickVisibleAt,
  dispatchFirstVisibleTextClick,
  fillVisibleAt,
  fillVisibleInputByLabel,
  visibleCount,
  waitForPageProperty,
  waitForVisibleElementsHidden,
  waitForVisibleInputByLabel,
} from '../../dom-helpers.mjs';
import { hoverAllTooltipTargetsInteraction } from '../../tooltip-helpers.mjs';
import {
  adminDrawerFooterButtonSelector,
  adminDrawerInputSelector,
  adminDrawerOpenSelector,
  adminMenuItemSelector,
  adminOrderMenuSelector,
  adminOverlayOpenSelector,
} from '../../selectors.mjs';

export async function runAdminOrderDetailModalInteraction(page) {
  // Open the first order's detail: the redesigned trade_no button shows the full
  // number, the antd oracle link shows the truncated `VIS...001`.
  await clickFirstVisibleText(page, '[data-testid^="order-open-"], .ant-table-tbody a', [
    'VISUAL2026110001',
    'VIS...001',
  ]);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      document.body.textContent.includes('订单信息') &&
      document.body.textContent.includes('VISUAL2026110001'),
    null,
    { timeout: 5_000 },
  );
  const opened = await adminOrderDetailModalState(page);
  // Both the antd Modal and the Radix Sheet close on Escape.
  await page.keyboard.press('Escape');
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  const closed = await adminOrderDetailModalState(page);
  return { closed, opened };
}

export async function runAdminOrderStatusTooltipsInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, [
    '[data-slot="header-tooltip-trigger"]',
    '.ant-table-thead .anticon-question-circle',
  ]);
}

export async function runAdminOrderAssignModalInteraction(page) {
  await clickFirstVisibleText(page, 'button, a', ['添加订单']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      document.body.textContent.includes('订单分配') &&
      document.body.textContent.includes('用户邮箱'),
    null,
    { timeout: 5_000 },
  );
  const opened = await adminOrderAssignModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'assign-user@example.com');
  await selectAdminOverlayOption(page, 0, 'Pro');
  await selectAdminOverlayOption(page, 1, '月付');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, '12.34');
  await page.waitForTimeout(100);
  const filled = await adminOrderAssignModalState(page);
  await clickVisibleAt(page, adminDrawerFooterButtonSelector, 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  const closed = await adminOrderAssignModalState(page);
  return {
    assignRequest: page.__visualParityLastAdminOrderAssign ?? null,
    closed,
    filled,
    opened,
  };
}

export async function runAdminOrderStatusDropdownInteraction(page) {
  const before = await adminOrderStatusDropdownState(page);
  await openAdminOrderRowTrigger(page, 'order-status-trigger-VISUAL2026110001', () =>
    clickFirstVisibleText(page, '.ant-table-tbody a', ['标记为']),
  );
  await page.waitForSelector(adminMenuItemSelector, { state: 'visible', timeout: 5_000 });
  const opened = await adminOrderStatusDropdownState(page);
  // Redesigned menu item text is `标记为已支付`; the antd item is exactly `已支付`.
  const markPaid = page.locator('[data-testid="order-mark-paid-VISUAL2026110001"]').first();
  if ((await markPaid.count()) > 0) {
    await markPaid.click();
  } else {
    await clickFirstVisibleText(page, adminMenuItemSelector, ['已支付']);
  }
  await waitForVisibleElementsHidden(page, adminOrderMenuSelector);
  const closed = await adminOrderStatusDropdownState(page);
  return {
    before,
    closed,
    opened,
    paidRequest: page.__visualParityLastAdminOrderPaid ?? null,
  };
}

export async function runAdminOrderCommissionDropdownInteraction(page) {
  const before = await adminOrderCommissionDropdownState(page);
  await openAdminOrderRowTrigger(page, 'commission-status-trigger-VISUAL2026110002', () =>
    clickAdminOrderRowAction(page, 'VIS...002', '标记为'),
  );
  await page.waitForSelector(adminMenuItemSelector, { state: 'visible', timeout: 5_000 });
  const opened = await adminOrderCommissionDropdownState(page);
  // The `无效` item text is identical in both DOMs, so exact-match click works.
  await clickFirstVisibleText(page, adminMenuItemSelector, ['无效']);
  await waitForVisibleElementsHidden(page, adminOrderMenuSelector);
  const closed = await adminOrderCommissionDropdownState(page);
  return {
    before,
    closed,
    opened,
    updateRequest: page.__visualParityLastAdminOrderUpdate ?? null,
  };
}

export async function runAdminOrdersFilterPaginationMatrixInteraction(page) {
  const before = await adminOrderFilterPaginationState(page);
  page.__visualParityLastAdminOrderFetchQuery = null;

  // Redesigned flow: the inline `order-search` box debounces (300ms) into
  // setFilter('trade_no','模糊', value); there is no antd `过滤器` drawer.
  if ((await visibleCount(page, '[data-testid="order-search"]')) > 0) {
    page.__visualParityDiagnostics?.push('admin orders matrix: type into order-search');
    await fillVisibleAt(page, '[data-testid="order-search"]', 0, 'VISUAL202611');
    await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
    await page.waitForTimeout(250);
    const filtered = await adminOrderFilterPaginationState(page);

    page.__visualParityLastAdminOrderFetchQuery = null;
    await page.waitForSelector('[data-testid="order-page"][data-page="2"]', {
      state: 'visible',
      timeout: 5_000,
    });
    page.__visualParityDiagnostics?.push('admin orders matrix: click page 2');
    await page.locator('[data-testid="order-page"][data-page="2"]').first().click();
    await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
    await page.waitForTimeout(250);
    const page2 = await adminOrderFilterPaginationState(page);

    return { before, filtered, page2 };
  }

  page.__visualParityDiagnostics?.push('admin orders matrix: click filter button');
  await clickFirstVisibleTextInViewport(page, '.bg-white .ant-btn, .ant-btn', ['过滤器']);
  await page.waitForSelector('.v2board-filter-drawer, .ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  page.__visualParityDiagnostics?.push('admin orders matrix: filter drawer opened');
  await dispatchFirstVisibleTextClick(page, '.v2board-filter-drawer .ant-btn', ['添加条件']);
  page.__visualParityDiagnostics?.push('admin orders matrix: condition added');
  await waitForVisibleInputByLabel(page, '.v2board-filter-drawer', '欲检索内容');
  await fillVisibleInputByLabel(page, '.v2board-filter-drawer', '欲检索内容', 'VISUAL202611');
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
      };
      const group = Array.from(document.querySelectorAll('.v2board-filter-drawer .form-group')).find(
        (element) =>
          isVisible(element) &&
          Array.from(element.querySelectorAll('label')).some((label) =>
            (label.textContent ?? '').includes('欲检索内容'),
          ),
      );
      const input = group
        ? Array.from(group.querySelectorAll('input, textarea')).find(
            (element) => isVisible(element) && !element.className.includes('ant-select-search__field'),
          )
        : null;
      return input && 'value' in input && input.value === 'VISUAL202611';
    },
    null,
    { timeout: 5_000 },
  );
  page.__visualParityDiagnostics?.push('admin orders matrix: filter value filled');
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('.v2board-filter-drawer .v2board-drawer-action .ant-btn')).some(
        (element) => {
          const text = (element.textContent ?? '').replace(/\s+/g, '');
          return (
            text.includes('检索') &&
            !element.hasAttribute('disabled') &&
            !element.className.includes('ant-btn-disabled')
          );
        },
      ),
    null,
    { timeout: 5_000 },
  );
  page.__visualParityDiagnostics?.push(
    `admin orders matrix: before search ${JSON.stringify(await filterDrawerDebugState(page))}`,
  );
  await dispatchFirstVisibleTextClick(page, '.v2board-filter-drawer .v2board-drawer-action .ant-btn', [
    '检索',
    '检 索',
  ]);
  await page.waitForTimeout(250);
  page.__visualParityDiagnostics?.push(
    `admin orders matrix: after search ${JSON.stringify(await filterDrawerDebugState(page))}`,
  );
  await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  page.__visualParityDiagnostics?.push('admin orders matrix: filter drawer closed');
  await page.waitForTimeout(250);
  const filtered = await adminOrderFilterPaginationState(page);

  page.__visualParityLastAdminOrderFetchQuery = null;
  await page.waitForSelector('.ant-pagination-item-2', { state: 'visible', timeout: 5_000 });
  page.__visualParityDiagnostics?.push('admin orders matrix: click page 2');
  await clickFirstVisible(page, '.ant-pagination-item-2');
  await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
  await page.waitForTimeout(250);
  const page2 = await adminOrderFilterPaginationState(page);

  return { before, filtered, page2 };
}
