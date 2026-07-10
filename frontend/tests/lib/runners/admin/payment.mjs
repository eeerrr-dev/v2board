import {
  clickFirstVisibleText,
  fillVisibleAt,
  openLegacySelectByLabel,
  clickFirstVisible,
  waitForVisibleElementsHidden,
  waitForPagePropertyAtLeast,
  waitForVisibleText,
  selectLegacyFormOption,
  fillFirstVisible,
  focusFirstVisible,
  keyboardFocusState,
} from '../../dom-helpers.mjs';
import { hoverAllTooltipTargetsInteraction } from '../../tooltip-helpers.mjs';
import {
  adminOverlayOpenSelector,
  adminDrawerInputSelector,
  adminSelectOptionSelector,
  adminSelectDropdownSelector,
  adminPaymentSaveSelector,
  adminDrawerTitleSelector,
  adminDrawerLabelSelector,
  scopedSelectorUnion,
} from '../../selectors.mjs';
import { clonePageRequests } from '../../json-util.mjs';
import {
  adminPaymentModalState,
  openAdminInlineRowEditor,
  clickAdminOrderRowAction,
} from '../../state-readers/admin.mjs';

export async function runAdminPaymentCreateModalInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  await clickFirstVisibleText(page, 'button', ['添加支付方式']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(() => document.body.textContent.includes('商户ID'), {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Pay');
  await page.waitForTimeout(100);
  const opened = await adminPaymentModalState(page);
  await openLegacySelectByLabel(page, adminOverlayOpenSelector, '接口文件');
  await page.waitForSelector(adminSelectOptionSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  const dropdown = await adminPaymentModalState(page);
  await clickFirstVisibleText(page, adminSelectOptionSelector, ['StripeCheckout']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await page.waitForFunction(() => document.body.textContent.includes('Secret Key'), {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminDrawerInputSelector, 5, 'pk_parity_create');
  await fillVisibleAt(page, adminDrawerInputSelector, 6, 'sk_parity_create');
  await page.waitForTimeout(100);
  const switched = await adminPaymentModalState(page);
  await clickFirstVisible(page, adminPaymentSaveSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPaymentSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPaymentFetchCount',
    initialPaymentFetchCount + 1,
  );
  const closed = await adminPaymentModalState(page);
  return {
    closed,
    dropdown,
    opened,
    paymentFetchDelta:
      (page.__visualParityAdminPaymentFetchCount ?? 0) - initialPaymentFetchCount,
    saveRequests: (page.__visualParityAdminPaymentSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    switched,
  };
}

export async function runAdminPaymentSaveFailureInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  await clickFirstVisibleText(page, 'button', ['添加支付方式']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(() => document.body.textContent.includes('商户ID'), {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Failed Pay');
  await page.waitForTimeout(100);
  const filled = await adminPaymentModalState(page);
  await clickFirstVisible(page, adminPaymentSaveSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPaymentSaveCount', 1);
  await page.waitForTimeout(350);
  const after = await adminPaymentModalState(page);
  return {
    after,
    filled,
    paymentFetchDelta:
      (page.__visualParityAdminPaymentFetchCount ?? 0) - initialPaymentFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminPaymentSaveRequests),
  };
}

export async function runAdminPaymentEditModalInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  const before = await adminPaymentModalState(page);
  await openAdminInlineRowEditor(page, 'Alipay', 'payment-edit-', () =>
    clickAdminOrderRowAction(page, 'Alipay', '编辑'),
  );
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑支付方式');
  await page.waitForFunction(
    (inputSelector) => {
      const values = Array.from(document.querySelectorAll(inputSelector)).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Alipay') && values.includes('visual-merchant');
    },
    adminDrawerInputSelector,
    { timeout: 5_000 },
  );
  const opened = await adminPaymentModalState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Edited Pay');
  await fillVisibleAt(page, adminDrawerInputSelector, 5, 'edited-secret');
  await fillVisibleAt(page, adminDrawerInputSelector, 6, 'edited-merchant');
  await page.waitForTimeout(100);
  const edited = await adminPaymentModalState(page);
  await clickFirstVisible(page, adminPaymentSaveSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPaymentSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPaymentFetchCount',
    initialPaymentFetchCount + 1,
  );
  const closed = await adminPaymentModalState(page);
  return {
    before,
    closed,
    edited,
    opened,
    paymentFetchDelta:
      (page.__visualParityAdminPaymentFetchCount ?? 0) - initialPaymentFetchCount,
    saveRequests: (page.__visualParityAdminPaymentSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminPaymentPluginFieldMatrixInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  await clickFirstVisibleText(page, 'button', ['添加支付方式']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(() => document.body.textContent.includes('商户ID'), {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Plugin Matrix');
  const alipay = await adminPaymentModalState(page);
  await selectLegacyFormOption(page, adminOverlayOpenSelector, '接口文件', ['MGate']);
  await waitForVisibleText(page, adminDrawerLabelSelector, 'Token');
  await fillFirstVisible(
    page,
    scopedSelectorUnion(adminOverlayOpenSelector, 'input[placeholder="请输入 MGate Token"]'),
    'mgate_matrix_token',
  );
  await page.waitForTimeout(100);
  const mgate = await adminPaymentModalState(page);
  await selectLegacyFormOption(page, adminOverlayOpenSelector, '接口文件', ['StripeCheckout']);
  await waitForVisibleText(page, adminDrawerLabelSelector, 'Secret Key');
  await fillFirstVisible(
    page,
    scopedSelectorUnion(adminOverlayOpenSelector, 'input[placeholder="请输入 Stripe Publishable Key"]'),
    'pk_matrix_plugin',
  );
  await fillFirstVisible(
    page,
    scopedSelectorUnion(adminOverlayOpenSelector, 'input[placeholder="请输入 Stripe Secret Key"]'),
    'sk_matrix_plugin',
  );
  await page.waitForTimeout(100);
  const stripe = await adminPaymentModalState(page);
  await clickFirstVisible(page, adminPaymentSaveSelector);
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPaymentSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPaymentFetchCount',
    initialPaymentFetchCount + 1,
  );
  const closed = await adminPaymentModalState(page);
  return {
    alipay,
    closed,
    mgate,
    paymentFetchDelta:
      (page.__visualParityAdminPaymentFetchCount ?? 0) - initialPaymentFetchCount,
    saveRequests: (page.__visualParityAdminPaymentSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    stripe,
  };
}

export async function runAdminPaymentModalKeyboardCloseInteraction(page) {
  const before = await adminPaymentModalState(page);
  await clickFirstVisibleText(page, 'button', ['添加支付方式']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '添加支付方式');
  const opened = await adminPaymentModalState(page);
  await focusFirstVisible(page, adminOverlayOpenSelector);
  const focused = await keyboardFocusState(page);
  await page.keyboard.press('Escape');
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  const closed = await adminPaymentModalState(page);
  return { before, closed, focused, opened };
}

export async function runAdminPaymentNotifyTooltipInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, [
    '.ant-table-thead .anticon-question-circle',
    '.v2board-service-tooltip-trigger',
  ]);
}
