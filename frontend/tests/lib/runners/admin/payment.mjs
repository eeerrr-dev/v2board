import {
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
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
  adminDrawerInputGroupControlSelector,
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

// Payment fee controls use the shadcn InputGroup slot while legacy antd exposes
// the same controls as ordinary inputs. Keep this union payment-local: selector
// lists are returned in document order, preserving the shared field indexes.
const adminPaymentInputSelector =
  `${adminDrawerInputSelector}, ${adminDrawerInputGroupControlSelector}`;

const paymentConfigInputSelector = (key, placeholder) =>
  scopedSelectorUnion(
    adminOverlayOpenSelector,
    `#payment-config-${key}, input[placeholder="${placeholder}"]`,
  );

const fillPaymentConfig = (page, key, placeholder, value) =>
  fillFirstVisible(page, paymentConfigInputSelector(key, placeholder), value);

export async function runAdminPaymentCreateModalInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  await clickFirstVisibleText(page, 'button', ['添加支付方式']);
  await page.waitForSelector(adminOverlayOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(() => document.body.textContent.includes('支付宝APPID'), null, {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminPaymentInputSelector, 0, 'Parity Pay');
  await page.waitForTimeout(100);
  const opened = await adminPaymentModalState(page);
  await openLegacySelectByLabel(page, adminOverlayOpenSelector, '接口文件');
  await page.waitForSelector(adminSelectOptionSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  const dropdown = await adminPaymentModalState(page);
  await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['StripeCheckout']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await page.waitForFunction(() => document.body.textContent.includes('SK_LIVE'), null, {
    timeout: 5_000,
  });
  await fillPaymentConfig(page, 'currency', '请输入货币单位', 'usd');
  await fillPaymentConfig(page, 'stripe_sk_live', 'API 密钥', 'sk_parity_create');
  await fillPaymentConfig(page, 'stripe_pk_live', 'API 公钥', 'pk_parity_create');
  await fillPaymentConfig(
    page,
    'stripe_webhook_key',
    '请输入 WebHook 密钥签名',
    'whsec_parity_create',
  );
  await fillPaymentConfig(
    page,
    'stripe_custom_field_name',
    '例如可设置为“联系方式”，以便及时与客户取得联系',
    'Contact',
  );
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
  await page.waitForFunction(() => document.body.textContent.includes('支付宝APPID'), null, {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminPaymentInputSelector, 0, 'Parity Failed Pay');
  // Fill the dynamic gateway fields explicitly. The frozen form displayed
  // backend-provided defaults without committing them to its submit state,
  // whereas the controlled form correctly submits displayed values. Explicit
  // input makes the failure request itself a shared, backend-valid Tier-1
  // payload instead of normalizing away that legacy defect.
  await fillPaymentConfig(page, 'app_id', '请输入支付宝 APPID', 'failed-app-id');
  await fillPaymentConfig(page, 'private_key', '请输入支付宝私钥', 'failed-private-key');
  await fillPaymentConfig(page, 'public_key', '请输入支付宝公钥', 'failed-public-key');
  await fillPaymentConfig(page, 'product_name', '将会体现在支付宝账单中', 'Failed Product');
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
      return values.includes('Alipay') && values.includes('visual-alipay-app');
    },
    adminPaymentInputSelector,
    { timeout: 5_000 },
  );
  const opened = await adminPaymentModalState(page);
  await fillVisibleAt(page, adminPaymentInputSelector, 0, 'Parity Edited Pay');
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
  await page.waitForFunction(() => document.body.textContent.includes('支付宝APPID'), null, {
    timeout: 5_000,
  });
  await fillVisibleAt(page, adminPaymentInputSelector, 0, 'Parity Plugin Matrix');
  const alipay = await adminPaymentModalState(page);
  await selectLegacyFormOption(page, adminOverlayOpenSelector, '接口文件', ['MGate']);
  await waitForVisibleText(page, adminDrawerLabelSelector, 'AppSecret');
  await fillPaymentConfig(page, 'mgate_url', '请输入 MGate API 地址', 'https://matrix.mgate.test');
  await fillPaymentConfig(page, 'mgate_app_id', '请输入 MGate APPID', 'mgate_matrix_app');
  await fillPaymentConfig(page, 'mgate_app_secret', '请输入 MGate AppSecret', 'mgate_matrix_secret');
  await fillPaymentConfig(page, 'mgate_source_currency', '请输入 MGate 源货币', 'CNY');
  await page.waitForTimeout(100);
  const mgate = await adminPaymentModalState(page);
  await selectLegacyFormOption(page, adminOverlayOpenSelector, '接口文件', ['StripeCheckout']);
  await waitForVisibleText(page, adminDrawerLabelSelector, 'SK_LIVE');
  await fillPaymentConfig(page, 'currency', '请输入货币单位', 'usd');
  await fillPaymentConfig(page, 'stripe_sk_live', 'API 密钥', 'sk_matrix_plugin');
  await fillPaymentConfig(page, 'stripe_pk_live', 'API 公钥', 'pk_matrix_plugin');
  await fillPaymentConfig(
    page,
    'stripe_webhook_key',
    '请输入 WebHook 密钥签名',
    'whsec_matrix_plugin',
  );
  await fillPaymentConfig(
    page,
    'stripe_custom_field_name',
    '例如可设置为“联系方式”，以便及时与客户取得联系',
    'Contact',
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
    '[data-slot="header-tooltip-trigger"]',
    '.ant-table-thead .anticon-question-circle',
  ]);
}
