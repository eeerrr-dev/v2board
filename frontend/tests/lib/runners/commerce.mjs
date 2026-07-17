import {
  plansFilterState,
  clickPlanFilterTab,
  clickCouponVerifyButton,
  commerceSummaryTexts,
  firstCommerceActionState,
  waitForOrderPaymentMethodCount,
  orderPaymentState,
  clickOrderPaymentMethodAt,
  orderCheckoutState,
  readStripeConfirmCount,
  waitForCreditCardSection,
} from '../state-readers/commerce.mjs';
import {
  visibleCount,
  fillFirstVisible,
  safeVisibleElementDomIndex,
  visibleTexts,
  firstInputValue,
  waitForPagePropertyAtLeast,
  clickFirstVisible,
  visibleTextCount,
  clickFirstVisibleText,
  waitForVisibleElementsHidden,
} from '../dom-helpers.mjs';
import { clonePageRequests } from '../json-util.mjs';
import {
  checkoutPeriodOptionSelector,
  checkoutCouponInputSelector,
  couponCheckFixture,
  checkoutCheckedPeriodOptionSelector,
  couponErrorCode,
} from '../fixture-data.mjs';

export async function runPlansFilterTabsInteraction(page) {
  const before = await plansFilterState(page);
  await clickPlanFilterTab(page, 1);
  await page.waitForTimeout(150);
  const period = await plansFilterState(page);
  await clickPlanFilterTab(page, 2);
  await page.waitForTimeout(150);
  const traffic = await plansFilterState(page);
  return { before, period, traffic };
}

export async function runPlanCheckoutCouponInteraction(page) {
  const selectCount = await visibleCount(page, checkoutPeriodOptionSelector);
  await fillFirstVisible(page, checkoutCouponInputSelector, couponCheckFixture.code);
  await clickCouponVerifyButton(page);
  await page
    .waitForFunction(
      (couponName) => document.body.textContent.includes(couponName),
      couponCheckFixture.name,
      {
        timeout: 5_000,
      },
    )
    .catch((error) => {
      // The frozen oracle may not render a coupon label even though the request
      // completed; the state reader below remains the authoritative comparison.
      void error;
    });

  return {
    activePeriodIndex: await safeVisibleElementDomIndex(
      page,
      checkoutCheckedPeriodOptionSelector,
      0,
    ),
    activePeriods: await visibleTexts(page, checkoutCheckedPeriodOptionSelector, 2),
    couponInput: await firstInputValue(page, checkoutCouponInputSelector),
    selectCount,
    summaryBlocks: await commerceSummaryTexts(
      page,
      '#cashier [data-testid="checkout-summary"], #cashier .col-md-4 .block',
      4,
    ),
    submitButton: await firstCommerceActionState(
      page,
      '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
    ),
  };
}

export async function runPlanCheckoutCouponErrorInteraction(page) {
  const initialCouponCheckCount = page.__visualParityUserCouponCheckCount ?? 0;
  const before = {
    activePeriods: await visibleTexts(page, checkoutCheckedPeriodOptionSelector, 2),
    summaryBlocks: await commerceSummaryTexts(
      page,
      '#cashier [data-testid="checkout-summary"], #cashier .col-md-4 .block',
      4,
    ),
  };
  await fillFirstVisible(page, checkoutCouponInputSelector, couponErrorCode);
  await clickCouponVerifyButton(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserCouponCheckCount',
    initialCouponCheckCount + 1,
  );
  await page.waitForTimeout(250);
  const after = {
    activePeriods: await visibleTexts(page, checkoutCheckedPeriodOptionSelector, 2),
    couponInput: await firstInputValue(page, checkoutCouponInputSelector),
    summaryBlocks: await commerceSummaryTexts(
      page,
      '#cashier [data-testid="checkout-summary"], #cashier .col-md-4 .block',
      4,
    ),
    submitButton: await firstCommerceActionState(
      page,
      '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
    ),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      4,
    ),
  };
  return {
    after,
    before,
    couponRequests: clonePageRequests(page.__visualParityUserCouponCheckRequests),
  };
}

export async function runOrderPaymentMethodInteraction(page) {
  await waitForOrderPaymentMethodCount(page);
  const before = await orderPaymentState(page);
  await clickOrderPaymentMethodAt(page, 2);
  await page.waitForTimeout(150);
  const after = await orderPaymentState(page);
  return { after, before };
}

export async function runOrderQrCheckoutInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  const before = await orderCheckoutState(page);
  await clickFirstVisible(
    page,
    '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
  );
  await page.waitForTimeout(100);
  const loading = await orderCheckoutState(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCheckoutCount',
    initialCheckoutCount + 1,
  );
  await page.waitForSelector('[data-testid="payment-qrcode"], .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () => /等待支付中|Waiting for payment/i.test(document.body.textContent ?? ''),
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(150);
  const opened = await orderCheckoutState(page);
  return {
    before,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    loading,
    opened,
  };
}

export async function runOrderCheckoutFailureInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  const before = await orderCheckoutState(page);
  await clickFirstVisible(
    page,
    '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
  );
  await page.waitForTimeout(100);
  const loading = await orderCheckoutState(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCheckoutCount',
    initialCheckoutCount + 1,
  );
  await page.waitForTimeout(250);
  const after = await orderCheckoutState(page);
  return {
    after,
    before,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    loading,
  };
}

export async function runOrderStripeDisabledCheckoutInteraction(page) {
  await waitForOrderPaymentMethodCount(page);
  const before = await orderCheckoutState(page);
  await clickOrderPaymentMethodAt(page, 1);
  await waitForPagePropertyAtLeast(page, '__visualParityUserStripePrepareCount', 1);
  await waitForCreditCardSection(page);
  await page.waitForTimeout(150);
  const selected = await orderCheckoutState(page);
  return {
    before,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    stripeIntentRequests: clonePageRequests(page.__visualParityUserStripeIntentRequests),
    selected,
  };
}

export async function runOrderStripePaymentIntentCheckoutInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  const initialConfirmCount = await readStripeConfirmCount(page);
  await waitForOrderPaymentMethodCount(page);
  const before = await orderCheckoutState(page);
  await clickOrderPaymentMethodAt(page, 1);
  await waitForPagePropertyAtLeast(page, '__visualParityUserStripePrepareCount', 1);
  await waitForCreditCardSection(page);
  await page.waitForFunction(
    () => {
      const button = document.querySelector(
        '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
      );
      return button instanceof HTMLButtonElement && !button.disabled;
    },
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(150);
  const selected = await orderCheckoutState(page);
  await clickFirstVisible(
    page,
    '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
  );
  await waitForStripeCheckoutAttempt(page, initialCheckoutCount, initialConfirmCount);
  await page.waitForTimeout(350);
  const checkedOut = await orderCheckoutState(page);
  return {
    before,
    checkedOut,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    stripeIntentRequests: clonePageRequests(page.__visualParityUserStripeIntentRequests),
    selected,
  };
}

export async function runOrderStripeConfirmationFailureInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  const initialConfirmCount = await readStripeConfirmCount(page);
  await waitForOrderPaymentMethodCount(page);
  const before = await orderCheckoutState(page);
  await clickOrderPaymentMethodAt(page, 1);
  await waitForPagePropertyAtLeast(page, '__visualParityUserStripePrepareCount', 1);
  await waitForCreditCardSection(page);
  await page.waitForFunction(
    () => {
      const button = document.querySelector(
        '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
      );
      return button instanceof HTMLButtonElement && !button.disabled;
    },
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(150);
  const selected = await orderCheckoutState(page);
  await clickFirstVisible(
    page,
    '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
  );
  await waitForStripeCheckoutAttempt(page, initialCheckoutCount, initialConfirmCount);
  await page.waitForTimeout(350);
  const after = await orderCheckoutState(page);
  return {
    after,
    before,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    stripeIntentRequests: clonePageRequests(page.__visualParityUserStripeIntentRequests),
    selected,
  };
}

async function waitForStripeCheckoutAttempt(
  page,
  initialCheckoutCount,
  initialConfirmCount,
  timeout = 5_000,
) {
  const startedAt = Date.now();
  while (Date.now() - startedAt <= timeout) {
    // The source confirms its server-owned PaymentIntent in the browser. The
    // frozen oracle instead posts its card token through the intercepted API,
    // whose counter belongs to the Node-side Page object.
    if (
      (page.__visualParityUserOrderCheckoutCount ?? 0) > initialCheckoutCount ||
      (await readStripeConfirmCount(page)) > initialConfirmCount
    ) {
      return;
    }
    await page.waitForTimeout(50);
  }
  throw new Error('Stripe checkout attempt was not observed');
}

export async function runOrderRedirectCheckoutInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  await waitForOrderPaymentMethodCount(page);
  await clickOrderPaymentMethodAt(page, 2);
  await page.waitForTimeout(100);
  const selected = await orderCheckoutState(page);
  await clickFirstVisible(
    page,
    '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
  );
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCheckoutCount',
    initialCheckoutCount + 1,
  );
  await page.waitForFunction(() => window.__parityReadSpaRoute().includes('cashier=visual'), null, {
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const redirected = await orderCheckoutState(page);
  return {
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    redirected,
    selected,
  };
}

export async function runOrderCancelConfirmInteraction(page) {
  const confirmSelector = '[data-slot="alert-dialog-content"], .ant-modal-confirm, .ant-modal';
  const confirmButtonSelector =
    '[data-slot="alert-dialog-content"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn';
  const confirmPrimarySelector =
    '[data-slot="alert-dialog-action"], .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary';
  const cancelActionSelector = 'a, button, [role="button"]';
  const cancelLinkTexts = ['Cancel', '取消'];
  const initialOrderCancelCount = page.__visualParityUserOrderCancelCount ?? 0;
  const initialOrderFetchCount = page.__visualParityUserOrderFetchCount ?? 0;
  const cancelLinks =
    (await visibleTextCount(page, cancelActionSelector, cancelLinkTexts)) > 0 ? 1 : 0;
  if (!cancelLinks) {
    return {
      cancelLinks,
      listItems: await visibleCount(page, '.am-list-item'),
      modalCount: await visibleCount(page, confirmSelector),
    };
  }

  await clickFirstVisibleText(page, cancelActionSelector, cancelLinkTexts);
  await page.waitForSelector(confirmSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const opened = {
    buttons: await visibleTexts(page, confirmButtonSelector, 4),
    content: await visibleTexts(
      page,
      '[data-slot="alert-dialog-description"], .ant-modal-confirm-content, .ant-modal-body',
      2,
    ),
    modalCount: await visibleCount(page, confirmSelector),
    title: await visibleTexts(
      page,
      '[data-slot="alert-dialog-title"], .ant-modal-confirm-title, .ant-modal-title',
      2,
    ),
  };

  await clickFirstVisible(page, confirmPrimarySelector);
  await waitForVisibleElementsHidden(page, confirmSelector);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCancelCount',
    initialOrderCancelCount + 1,
  );
  await page.waitForTimeout(150);

  return {
    cancelLinks,
    confirmed: {
      modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    },
    opened,
    orderCancelRequests: (page.__visualParityUserOrderCancelRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    orderFetchDelta: (page.__visualParityUserOrderFetchCount ?? 0) - initialOrderFetchCount,
  };
}
