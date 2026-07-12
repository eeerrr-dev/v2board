import {
  openAdminCreateOverlay,
  fillAdminOverlayInput,
  adminCouponModalState,
  clickAdminEntitySubmit,
  adminUserFilterDateFieldState,
  selectAdminOverlayOption,
  toggleAdminCouponScopeItem,
  clickAdminRowEditControl,
  adminGiftcardModalState,
  openAdminOverlaySelectTrigger,
  fillAdminOverlayInputBySelector,
  adminNoticeModalState,
  fillAdminNoticeField,
  addAdminNoticeTag,
  adminKnowledgeDrawerState,
} from '../../state-readers/admin.mjs';
import {
  waitForVisibleText,
  waitForPagePropertyAtLeast,
  waitForVisibleElementsHidden,
  clickFirstVisible,
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
  fillVisibleAt,
} from '../../dom-helpers.mjs';
import { clonePageRequests } from '../../json-util.mjs';
import {
  adminDrawerTitleSelector,
  adminOverlayOpenSelector,
  adminDrawerInputSelector,
  adminSelectOptionSelector,
  adminSelectDropdownSelector,
  adminDrawerFooterButtonSelector,
  adminDrawerOpenSelector,
} from '../../selectors.mjs';

async function fillRedesignedValidityWindow(page, prefix) {
  const start = page.locator(`[data-testid="${prefix}-start"]`).first();
  const end = page.locator(`[data-testid="${prefix}-end"]`).first();
  if ((await start.count()) === 0 || (await end.count()) === 0) return;
  await start.fill('2030-01-01T00:00');
  await end.fill('2030-01-02T00:00');
}

async function finishAdminKnowledgeSave(page) {
  // The redesigned editor closes itself after its invalidating mutation has
  // settled; the frozen oracle leaves its drawer open. Detect the outcome
  // instead of branching by world. The grace period also prevents a Cancel
  // coordinate racing the source's closing animation and landing on Submit.
  try {
    await waitForVisibleElementsHidden(page, adminDrawerOpenSelector, 1_500);
  } catch {
    await clickFirstVisibleTextStable(page, adminDrawerFooterButtonSelector, ['取消', '取 消']);
  }
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
  await waitForVisibleElementsHidden(page, adminDrawerTitleSelector);
}

export async function runAdminCouponCreateModalInteraction(page) {
  const initialCouponFetchCount = page.__visualParityAdminCouponFetchCount ?? 0;
  await openAdminCreateOverlay(page, 'coupon-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建优惠券');
  await fillAdminOverlayInput(page, 'coupon-name', 0, 'Parity Coupon');
  await fillAdminOverlayInput(page, 'coupon-code', 1, 'PARITY2026');
  await fillAdminOverlayInput(page, 'coupon-value', 2, '25');
  await fillRedesignedValidityWindow(page, 'coupon');
  await page.waitForTimeout(100);
  const opened = await adminCouponModalState(page);
  await clickAdminEntitySubmit(page, 'coupon-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminCouponGenerateCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminCouponFetchCount',
    initialCouponFetchCount + 1,
  );
  const closed = await adminCouponModalState(page);
  return {
    closed,
    couponFetchDelta: (page.__visualParityAdminCouponFetchCount ?? 0) - initialCouponFetchCount,
    generateRequests: (page.__visualParityAdminCouponGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    opened,
  };
}

export async function runAdminCouponGenerateFailureInteraction(page) {
  const initialCouponFetchCount = page.__visualParityAdminCouponFetchCount ?? 0;
  await openAdminCreateOverlay(page, 'coupon-create');
  await fillAdminOverlayInput(page, 'coupon-name', 0, 'Parity Failed Coupon');
  await fillAdminOverlayInput(page, 'coupon-code', 1, 'FAIL2026');
  await fillAdminOverlayInput(page, 'coupon-value', 2, '25');
  await fillRedesignedValidityWindow(page, 'coupon');
  await page.waitForTimeout(100);
  const filled = await adminCouponModalState(page);
  await clickAdminEntitySubmit(page, 'coupon-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminCouponGenerateCount', 1);
  await page.waitForTimeout(350);
  const after = await adminCouponModalState(page);
  return {
    after,
    couponFetchDelta: (page.__visualParityAdminCouponFetchCount ?? 0) - initialCouponFetchCount,
    filled,
    generateRequests: clonePageRequests(page.__visualParityAdminCouponGenerateRequests),
  };
}

export async function runAdminCouponRangePickerInteraction(page) {
  await openAdminCreateOverlay(page, 'coupon-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建优惠券');
  const before = await adminUserFilterDateFieldState(page, 'coupon-');
  // The redesigned editor exposes the validity window as two native
  // datetime-local inputs (coupon-start/coupon-end); the antd oracle opens a
  // range-picker calendar popup. The popup chrome is Tier-2 presentation, so both
  // reduce to whether the validity-window date fields are reachable in the editor.
  const shadcnStart = page.locator('[data-testid="coupon-start"]').first();
  if ((await shadcnStart.count()) > 0) {
    await shadcnStart.click().catch(() => undefined);
  } else {
    await clickFirstVisible(page, '.ant-modal .ant-calendar-range-picker-input');
    await page.waitForSelector('.ant-calendar-picker-container', {
      state: 'visible',
      timeout: 5_000,
    });
  }
  await page.waitForTimeout(150);
  const opened = await adminUserFilterDateFieldState(page, 'coupon-');
  return { before, opened };
}

export async function runAdminCouponTypeMatrixInteraction(page) {
  const initialCouponFetchCount = page.__visualParityAdminCouponFetchCount ?? 0;
  await openAdminCreateOverlay(page, 'coupon-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建优惠券');
  await fillAdminOverlayInput(page, 'coupon-name', 0, 'Parity Ratio Coupon');
  await fillAdminOverlayInput(page, 'coupon-code', 1, 'RATIO2026');
  await fillAdminOverlayInput(page, 'coupon-value', 2, '15');
  await fillRedesignedValidityWindow(page, 'coupon');
  const amount = await adminCouponModalState(page);
  // The 优惠信息 type control is the first overlay select in both worlds (the
  // redesigned coupon-type Radix Select; the antd single-select).
  await selectAdminOverlayOption(page, 0, '按比例优惠');
  await page.waitForTimeout(100);
  const ratio = await adminCouponModalState(page);
  await toggleAdminCouponScopeItem(page, 'coupon-plan-ids', 'Pro', '指定订阅');
  await toggleAdminCouponScopeItem(page, 'coupon-periods', '月付', '指定周期');
  await page.waitForTimeout(100);
  const limited = await adminCouponModalState(page);
  await clickAdminEntitySubmit(page, 'coupon-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminCouponGenerateCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminCouponFetchCount',
    initialCouponFetchCount + 1,
  );
  const closed = await adminCouponModalState(page);
  return {
    amount,
    closed,
    couponFetchDelta: (page.__visualParityAdminCouponFetchCount ?? 0) - initialCouponFetchCount,
    generateRequests: (page.__visualParityAdminCouponGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    limited,
    ratio,
  };
}

export async function runAdminCouponEditModalInteraction(page) {
  const initialCouponFetchCount = page.__visualParityAdminCouponFetchCount ?? 0;
  const before = await adminCouponModalState(page);
  await clickAdminRowEditControl(page, 'Visual Amount');
  await page.waitForSelector(adminOverlayOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑优惠券');
  await page.waitForFunction(
    ({ selector }) => {
      const values = Array.from(document.querySelectorAll(selector)).map((element) =>
        'value' in element ? element.value : '',
      );
      return values.includes('Visual Amount') && values.includes('VISUAL100');
    },
    { selector: adminDrawerInputSelector },
    { timeout: 5_000 },
  );
  const opened = await adminCouponModalState(page);
  await fillAdminOverlayInput(page, 'coupon-name', 0, 'Parity Edited Coupon');
  await fillAdminOverlayInput(page, 'coupon-code', 1, 'EDIT2026');
  await fillAdminOverlayInput(page, 'coupon-value', 2, '12.5');
  await page.waitForTimeout(100);
  const edited = await adminCouponModalState(page);
  await clickAdminEntitySubmit(page, 'coupon-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminCouponGenerateCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminCouponFetchCount',
    initialCouponFetchCount + 1,
  );
  const closed = await adminCouponModalState(page);
  return {
    before,
    closed,
    couponFetchDelta: (page.__visualParityAdminCouponFetchCount ?? 0) - initialCouponFetchCount,
    edited,
    generateRequests: (page.__visualParityAdminCouponGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    opened,
  };
}

export async function runAdminGiftcardCreateModalInteraction(page) {
  const initialGiftcardFetchCount = page.__visualParityAdminGiftcardFetchCount ?? 0;
  const before = await adminGiftcardModalState(page);
  await openAdminCreateOverlay(page, 'giftcard-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建礼品卡');
  await fillAdminOverlayInput(page, 'giftcard-name', 0, 'Parity Giftcard');
  await fillAdminOverlayInput(page, 'giftcard-code', 1, 'GIFT2026');
  const opened = await adminGiftcardModalState(page);
  await openAdminOverlaySelectTrigger(page, 'giftcard-type', 0);
  await waitForVisibleText(page, adminSelectOptionSelector, '兑换订阅套餐');
  const typeDropdown = await adminGiftcardModalState(page);
  await clickFirstVisibleText(page, adminSelectOptionSelector, ['兑换订阅套餐']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await fillAdminOverlayInputBySelector(
    page,
    'giftcard-value',
    '.ant-modal input[placeholder="一次性套餐输入0"]',
    '0',
  );
  await fillRedesignedValidityWindow(page, 'giftcard');
  await openAdminOverlaySelectTrigger(page, 'giftcard-plan', 1);
  await waitForVisibleText(page, adminSelectOptionSelector, 'Pro');
  const planDropdown = await adminGiftcardModalState(page);
  await clickFirstVisibleText(page, adminSelectOptionSelector, ['Pro']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await fillAdminOverlayInputBySelector(
    page,
    'giftcard-limit-use',
    '.ant-modal input[placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"]',
    '9',
  );
  await page.waitForTimeout(100);
  const filled = await adminGiftcardModalState(page);
  await clickAdminEntitySubmit(page, 'giftcard-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminGiftcardGenerateCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminGiftcardFetchCount',
    initialGiftcardFetchCount + 1,
  );
  const closed = await adminGiftcardModalState(page);
  return {
    before,
    closed,
    filled,
    generateRequests: (page.__visualParityAdminGiftcardGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    giftcardFetchDelta:
      (page.__visualParityAdminGiftcardFetchCount ?? 0) - initialGiftcardFetchCount,
    opened,
    planDropdown,
    typeDropdown,
  };
}

export async function runAdminGiftcardGenerateFailureInteraction(page) {
  const initialGiftcardFetchCount = page.__visualParityAdminGiftcardFetchCount ?? 0;
  const before = await adminGiftcardModalState(page);
  await openAdminCreateOverlay(page, 'giftcard-create');
  await fillAdminOverlayInput(page, 'giftcard-name', 0, 'Parity Failed Giftcard');
  await fillAdminOverlayInput(page, 'giftcard-code', 1, 'FAIL-GIFT-2026');
  await fillAdminOverlayInputBySelector(
    page,
    'giftcard-value',
    '.ant-modal input[placeholder="请输入值"]',
    '10',
  );
  await fillRedesignedValidityWindow(page, 'giftcard');
  await page.waitForTimeout(100);
  const filled = await adminGiftcardModalState(page);
  await clickAdminEntitySubmit(page, 'giftcard-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminGiftcardGenerateCount', 1);
  await page.waitForTimeout(350);
  const after = await adminGiftcardModalState(page);
  return {
    after,
    before,
    filled,
    generateRequests: clonePageRequests(page.__visualParityAdminGiftcardGenerateRequests),
    giftcardFetchDelta:
      (page.__visualParityAdminGiftcardFetchCount ?? 0) - initialGiftcardFetchCount,
  };
}

export async function runAdminGiftcardEditModalInteraction(page) {
  const initialGiftcardFetchCount = page.__visualParityAdminGiftcardFetchCount ?? 0;
  const before = await adminGiftcardModalState(page);
  await clickAdminRowEditControl(page, 'Plan Gift');
  await page.waitForSelector(adminOverlayOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑礼品卡');
  await page.waitForFunction(
    ({ selector }) => {
      const values = Array.from(document.querySelectorAll(selector)).map((element) =>
        'value' in element ? element.value : '',
      );
      return values.includes('Plan Gift') && values.includes('GC-VISUAL-PLAN');
    },
    { selector: adminDrawerInputSelector },
    { timeout: 5_000 },
  );
  const opened = await adminGiftcardModalState(page);
  await fillAdminOverlayInput(page, 'giftcard-name', 0, 'Parity Edited Giftcard');
  await fillAdminOverlayInput(page, 'giftcard-code', 1, 'EDIT-GIFT-2026');
  await fillAdminOverlayInputBySelector(
    page,
    'giftcard-value',
    '.ant-modal input[placeholder="一次性套餐输入0"]',
    '45',
  );
  await fillAdminOverlayInputBySelector(
    page,
    'giftcard-limit-use',
    '.ant-modal input[placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"]',
    '4',
  );
  await page.waitForTimeout(100);
  const edited = await adminGiftcardModalState(page);
  await clickAdminEntitySubmit(page, 'giftcard-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminGiftcardGenerateCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminGiftcardFetchCount',
    initialGiftcardFetchCount + 1,
  );
  const closed = await adminGiftcardModalState(page);
  return {
    before,
    closed,
    edited,
    generateRequests: (page.__visualParityAdminGiftcardGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    giftcardFetchDelta:
      (page.__visualParityAdminGiftcardFetchCount ?? 0) - initialGiftcardFetchCount,
    opened,
  };
}

export async function runAdminNoticeCreateModalInteraction(page) {
  const initialNoticeFetchCount = page.__visualParityAdminNoticeFetchCount ?? 0;
  const before = await adminNoticeModalState(page);
  await openAdminCreateOverlay(page, 'notice-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建公告');
  await fillAdminNoticeField(page, '#notice-title', '.ant-modal .ant-input', 0, 'Parity Notice');
  await fillAdminNoticeField(
    page,
    '#notice-content',
    '.ant-modal textarea.ant-input',
    0,
    'Parity notice body',
  );
  await addAdminNoticeTag(page, 'ops');
  await fillAdminNoticeField(
    page,
    '#notice-img',
    '.ant-modal .ant-input',
    2,
    'https://example.test/notice.png',
  );
  await page.waitForTimeout(100);
  const filled = await adminNoticeModalState(page);
  await clickAdminEntitySubmit(page, 'notice-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminNoticeFetchCount',
    initialNoticeFetchCount + 1,
  );
  const closed = await adminNoticeModalState(page);
  await openAdminCreateOverlay(page, 'notice-create');
  const reopened = await adminNoticeModalState(page);
  await clickFirstVisibleText(page, adminDrawerFooterButtonSelector, ['取消']);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  return {
    before,
    closed,
    filled,
    noticeFetchDelta: (page.__visualParityAdminNoticeFetchCount ?? 0) - initialNoticeFetchCount,
    reopened,
    saveRequests: (page.__visualParityAdminNoticeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminNoticeSaveFailureInteraction(page) {
  const initialNoticeFetchCount = page.__visualParityAdminNoticeFetchCount ?? 0;
  const before = await adminNoticeModalState(page);
  await openAdminCreateOverlay(page, 'notice-create');
  await fillAdminNoticeField(page, '#notice-title', '.ant-modal .ant-input', 0, 'Parity Failed Notice');
  await fillAdminNoticeField(
    page,
    '#notice-content',
    '.ant-modal textarea.ant-input',
    0,
    'Parity notice failure body',
  );
  await addAdminNoticeTag(page, 'failure');
  await page.waitForTimeout(100);
  const filled = await adminNoticeModalState(page);
  await clickAdminEntitySubmit(page, 'notice-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeSaveCount', 1);
  await page.waitForTimeout(350);
  const after = await adminNoticeModalState(page);
  return {
    after,
    before,
    filled,
    noticeFetchDelta: (page.__visualParityAdminNoticeFetchCount ?? 0) - initialNoticeFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminNoticeSaveRequests),
  };
}

export async function runAdminNoticeEditModalInteraction(page) {
  const initialNoticeFetchCount = page.__visualParityAdminNoticeFetchCount ?? 0;
  const before = await adminNoticeModalState(page);
  await clickAdminRowEditControl(page, 'Hidden Notice');
  await page.waitForSelector(adminOverlayOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑公告');
  await page.waitForFunction(
    ({ selector }) => {
      const values = Array.from(document.querySelectorAll(selector)).map((element) =>
        'value' in element ? element.value : '',
      );
      return values.includes('Hidden Notice') && values.includes('<p>Second notice</p>');
    },
    { selector: adminDrawerInputSelector },
    { timeout: 5_000 },
  );
  const opened = await adminNoticeModalState(page);
  await fillAdminNoticeField(page, '#notice-title', '.ant-modal .ant-input', 0, 'Parity Edited Notice');
  await fillAdminNoticeField(
    page,
    '#notice-content',
    '.ant-modal textarea.ant-input',
    0,
    '<p>Parity edited notice body</p>',
  );
  await addAdminNoticeTag(page, 'edited');
  await fillAdminNoticeField(
    page,
    '#notice-img',
    '.ant-modal .ant-input',
    2,
    'https://example.test/notice-edited.png',
  );
  await page.waitForTimeout(100);
  const edited = await adminNoticeModalState(page);
  await clickAdminEntitySubmit(page, 'notice-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeSaveCount', 1);
  await waitForVisibleElementsHidden(page, adminOverlayOpenSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminNoticeFetchCount',
    initialNoticeFetchCount + 1,
  );
  const closed = await adminNoticeModalState(page);
  return {
    before,
    closed,
    edited,
    noticeFetchDelta: (page.__visualParityAdminNoticeFetchCount ?? 0) - initialNoticeFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminNoticeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminKnowledgeCreateDrawerInteraction(page) {
  const initialKnowledgeFetchCount = page.__visualParityAdminKnowledgeFetchCount ?? 0;
  const before = await adminKnowledgeDrawerState(page);
  await openAdminCreateOverlay(page, 'knowledge-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新增知识');
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Knowledge');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, 'Parity');
  await openAdminOverlaySelectTrigger(page, 'knowledge-language', 0);
  await waitForVisibleText(page, adminSelectOptionSelector, 'English');
  const languageDropdown = await adminKnowledgeDrawerState(page);
  await clickFirstVisibleText(page, adminSelectOptionSelector, ['English']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await fillAdminOverlayInputBySelector(
    page,
    'knowledge-body',
    '.ant-drawer-open textarea.section-container.input',
    '# Parity Knowledge\n\nParity body',
  );
  await page.waitForTimeout(100);
  const filled = await adminKnowledgeDrawerState(page);
  await clickAdminEntitySubmit(page, 'knowledge-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminKnowledgeSaveCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminKnowledgeFetchCount',
    initialKnowledgeFetchCount + 1,
  );
  await finishAdminKnowledgeSave(page);
  const closed = await adminKnowledgeDrawerState(page);
  return {
    before,
    closed,
    filled,
    knowledgeFetchDelta:
      (page.__visualParityAdminKnowledgeFetchCount ?? 0) - initialKnowledgeFetchCount,
    languageDropdown,
    saveRequests: (page.__visualParityAdminKnowledgeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

export async function runAdminKnowledgeSaveFailureInteraction(page) {
  const initialKnowledgeFetchCount = page.__visualParityAdminKnowledgeFetchCount ?? 0;
  const before = await adminKnowledgeDrawerState(page);
  await openAdminCreateOverlay(page, 'knowledge-create');
  await waitForVisibleText(page, adminDrawerTitleSelector, '新增知识');
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Failed Knowledge');
  await fillVisibleAt(page, adminDrawerInputSelector, 1, 'Parity');
  await openAdminOverlaySelectTrigger(page, 'knowledge-language', 0);
  await waitForVisibleText(page, adminSelectOptionSelector, 'English');
  await clickFirstVisibleText(page, adminSelectOptionSelector, ['English']);
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
  await fillAdminOverlayInputBySelector(
    page,
    'knowledge-body',
    '.ant-drawer-open textarea.section-container.input',
    '# Parity Failed Knowledge\n\nFailure body',
  );
  await page.waitForTimeout(100);
  const filled = await adminKnowledgeDrawerState(page);
  await clickAdminEntitySubmit(page, 'knowledge-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminKnowledgeSaveCount', 1);
  await page.waitForTimeout(350);
  const after = await adminKnowledgeDrawerState(page);
  return {
    after,
    before,
    filled,
    knowledgeFetchDelta:
      (page.__visualParityAdminKnowledgeFetchCount ?? 0) - initialKnowledgeFetchCount,
    saveRequests: clonePageRequests(page.__visualParityAdminKnowledgeSaveRequests),
  };
}

export async function runAdminKnowledgeEditDrawerInteraction(page) {
  const initialKnowledgeFetchCount = page.__visualParityAdminKnowledgeFetchCount ?? 0;
  const before = await adminKnowledgeDrawerState(page);
  await clickAdminRowEditControl(page, 'Copy Article');
  await page.waitForSelector(adminDrawerOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '编辑知识');
  await page.waitForFunction(
    ({ selector }) =>
      Array.from(document.querySelectorAll(selector)).some(
        (element) => 'value' in element && element.value === 'Copy Article',
      ),
    { selector: adminDrawerInputSelector },
    { timeout: 5_000 },
  );
  const opened = await adminKnowledgeDrawerState(page);
  await fillVisibleAt(page, adminDrawerInputSelector, 0, 'Parity Edited Article');
  await fillAdminOverlayInputBySelector(
    page,
    'knowledge-body',
    '.ant-drawer-open textarea.section-container.input',
    '## Parity Edited Article\n\nEdited body',
  );
  await page.waitForTimeout(100);
  const edited = await adminKnowledgeDrawerState(page);
  await clickAdminEntitySubmit(page, 'knowledge-submit');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminKnowledgeSaveCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminKnowledgeFetchCount',
    initialKnowledgeFetchCount + 1,
  );
  await finishAdminKnowledgeSave(page);
  const closed = await adminKnowledgeDrawerState(page);
  return {
    before,
    closed,
    edited,
    knowledgeFetchDelta:
      (page.__visualParityAdminKnowledgeFetchCount ?? 0) - initialKnowledgeFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminKnowledgeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}
