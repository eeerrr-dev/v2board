import { orderPaymentMethodNames } from '../fixture-data.mjs';
import { visibleCount, visibleTexts, firstElementState } from '../dom-helpers.mjs';

const planFilterControlSelector =
  '[data-testid="plan-tabs"] [role="tab"], [data-testid="plan-tabs"] [role="radio"]';

export async function orderPaymentState(page) {
  const paymentOptions = await orderPaymentOptionStates(page);
  const detectedActiveIndex = paymentOptions.findIndex((option) => option.checked);
  return {
    activeIndex:
      detectedActiveIndex >= 0
        ? detectedActiveIndex
        : (page.__visualParitySelectedPaymentIndex ?? 0),
    methodTexts: paymentOptions.length
      ? paymentOptions.map((option) => option.name)
      : orderPaymentMethodNames,
    summaryBlocks: await commerceSummaryTexts(
      page,
      '#cashier [data-testid="order-summary"], #cashier [data-testid="checkout-summary"], #cashier .v2board-order-summary, #cashier .col-md-4 .block',
      4,
    ),
    submitButton: await firstCommerceActionState(
      page,
      '#cashier [data-testid="commerce-submit"], #cashier .btn-block.btn-primary',
    ),
  };
}

export async function orderPaymentOptionStates(page) {
  return page.evaluate((methodNames) => {
    const normalizeText = (value) =>
      String(value ?? '')
        .trim()
        .replace(/\s+/g, ' ');
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        !element.closest('.ant-dropdown-hidden')
      );
    };
    const matchesMethod = (text) => methodNames.filter((name) => text.includes(name));
    const candidates = Array.from(
      document.querySelectorAll(
        '#cashier [data-testid="payment-option"], #cashier [role="radio"], #cashier .ant-radio-button-wrapper, #cashier .ant-radio-wrapper, #cashier label',
      ),
    )
      .filter(isVisible)
      .map((element) => {
        const text = normalizeText(element.textContent);
        const matchedNames = matchesMethod(text);
        return { element, matchedNames, text };
      })
      .filter(({ matchedNames }) => matchedNames.length === 1);

    return methodNames
      .map((name) => candidates.find(({ matchedNames }) => matchedNames[0] === name))
      .filter(Boolean)
      .map(({ element, matchedNames, text }) => {
        const input = element.querySelector('input[type="radio"]');
        const state = element.getAttribute('data-state');
        const ariaChecked = element.getAttribute('aria-checked');
        return {
          checked:
            state === 'checked' ||
            ariaChecked === 'true' ||
            element.matches(
              '.active, .ant-radio-button-wrapper-checked, .ant-radio-wrapper-checked',
            ) ||
            Boolean(element.querySelector('.ant-radio-checked')) ||
            Boolean(input?.checked),
          name: matchedNames[0],
          text,
        };
      });
  }, orderPaymentMethodNames);
}

export async function orderCheckoutState(page) {
  return {
    ...(await orderPaymentState(page)),
    creditCardTexts: await commerceCreditCardTexts(page),
    hash: await page.evaluate(() => window.__parityReadSpaRoute()),
    modalCount: await visibleCount(page, '[data-testid="payment-qrcode"], .ant-modal'),
    modalTexts: await visibleTexts(
      page,
      '[data-testid="payment-qrcode-status"], [data-testid="payment-qrcode"], .ant-modal',
      4,
    ),
    qrCanvasCount: await visibleCount(
      page,
      '[data-testid="payment-qrcode"] canvas, .ant-modal canvas',
    ),
    qrSvgCount: await visibleCount(page, '[data-testid="payment-qrcode"] svg, .ant-modal svg'),
    stripePublicKeyCount: page.__visualParityUserStripePublicKeyCount ?? 0,
    stripeIntentCount: page.__visualParityUserStripeIntentCount ?? 0,
    stripeConfirmCount: await readStripeConfirmCount(page),
    stripeUnexpectedCreateTokenCount: await page.evaluate(
      () => window.__visualParityUnexpectedStripeCreateTokenCount ?? 0,
    ),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      4,
    ),
  };
}

// API request counters live on Playwright's Node-side Page object, while the
// Stripe fixture runs inside the browser and records confirmPayment there.
// Keep this boundary explicit so a real PaymentIntent confirmation cannot be
// mistaken for the frozen oracle's legacy /order/checkout request.
export function readStripeConfirmCount(page) {
  return page.evaluate(() => window.__visualParityUserStripeConfirmCount ?? 0);
}

export async function waitForOrderPaymentMethodCount(page) {
  await page.waitForFunction(
    (methodNames) => {
      const text =
        document.querySelector('#cashier')?.textContent ?? document.body.textContent ?? '';
      return methodNames.every((name) => text.includes(name));
    },
    orderPaymentMethodNames,
    { timeout: 5_000 },
  );
}

export async function clickOrderPaymentMethodAt(page, index) {
  const point = await page.evaluate(
    ({ index: targetIndex, methodNames }) => {
      const normalizeText = (value) =>
        String(value ?? '')
          .trim()
          .replace(/\s+/g, ' ');
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          !element.closest('.ant-dropdown-hidden')
        );
      };
      const targetName = methodNames[targetIndex];
      const exactCandidates = Array.from(
        document.querySelectorAll(
          '#cashier [data-testid="payment-option"], #cashier [role="radio"], #cashier .ant-radio-button-wrapper, #cashier .ant-radio-wrapper, #cashier label',
        ),
      ).find((candidate) => {
        const text = normalizeText(candidate.textContent);
        const matchedNames = methodNames.filter((name) => text.includes(name));
        return isVisible(candidate) && matchedNames.length === 1 && matchedNames[0] === targetName;
      });
      const element =
        exactCandidates ??
        Array.from(document.querySelectorAll('#cashier *'))
          .filter((candidate) => {
            const text = normalizeText(candidate.textContent);
            const matchedNames = methodNames.filter((name) => text.includes(name));
            return (
              isVisible(candidate) && matchedNames.length === 1 && matchedNames[0] === targetName
            );
          })
          .sort(
            (left, right) =>
              normalizeText(left.textContent).length - normalizeText(right.textContent).length,
          )[0];
      if (!element) {
        throw new Error(`No visible payment method at index ${targetIndex}`);
      }
      element.scrollIntoView({ block: 'center', inline: 'center' });
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    },
    { index, methodNames: orderPaymentMethodNames },
  );
  await page.mouse.click(point.x, point.y);
  page.__visualParitySelectedPaymentIndex = index;
}

export async function waitForCreditCardSection(page) {
  await page.waitForFunction(
    () => {
      const text = document.querySelector('#cashier')?.textContent ?? '';
      return /信用卡|credit card/i.test(text);
    },
    null,
    { timeout: 5_000 },
  );
}

export async function commerceCreditCardTexts(page) {
  const texts = await visibleTexts(
    page,
    '#cashier h2, #cashier h3, #cashier .fa-user-shield, #cashier .mt-3.mb-5',
    8,
  );
  return texts.filter((text) => /信用卡|credit card|安全|secure|security|encrypt|加密/i.test(text));
}

export async function plansFilterState(page) {
  return {
    activeIndex: await activePlanTabIndex(page),
    cardCount: await visibleCount(page, '[data-testid="plan-card"], a.block-link-pop'),
    cardTitles: await visibleTexts(
      page,
      '[data-testid="plan-card-title"], .block-header.plan .block-title',
      6,
    ),
    tabStates: await planTabStates(page),
  };
}

export async function activePlanTabIndex(page) {
  return page.evaluate((modernSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const planTabLabels = [
      '全部',
      'All',
      '按周期',
      'By Period',
      'Period',
      '按流量',
      'By Traffic',
      'Traffic',
    ];
    const isPlanTabLabel = (element) =>
      planTabLabels.includes((element.textContent ?? '').trim().replace(/\s+/g, ' '));
    const isActiveTab = (element) =>
      element.getAttribute('data-state') === 'active' ||
      element.getAttribute('data-state') === 'checked' ||
      element.getAttribute('aria-selected') === 'true' ||
      element.getAttribute('aria-checked') === 'true' ||
      String(element.className).split(/\s+/).includes('active') ||
      Boolean(
        element.closest(
          '.ant-tabs-tab-active, .ant-radio-button-wrapper-checked, .ant-segmented-item-selected',
        ),
      );
    const modernTabs = Array.from(document.querySelectorAll(modernSelector)).filter(isVisible);
    const tabs = modernTabs.length
      ? modernTabs
      : Array.from(
          document.querySelectorAll(
            '[data-testid="plan-tabs"] span, .ant-tabs-tab, .ant-radio-button-wrapper, .ant-segmented-item, [role="tab"], span, button',
          ),
        ).filter((element) => isVisible(element) && isPlanTabLabel(element));
    return tabs.findIndex(isActiveTab);
  }, planFilterControlSelector);
}

export async function clickPlanFilterTab(page, index) {
  const modernCount = await visibleCount(page, planFilterControlSelector);
  if (modernCount > 0) {
    await page.evaluate(
      ({ index: targetIndex, selector: targetSelector }) => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const dispatchSequence = (element) => {
          const pointerEvent =
            typeof PointerEvent === 'function'
              ? new PointerEvent('pointerdown', {
                  bubbles: true,
                  button: 0,
                  cancelable: true,
                  pointerType: 'mouse',
                })
              : new MouseEvent('mousedown', {
                  bubbles: true,
                  button: 0,
                  cancelable: true,
                });
          element.dispatchEvent(pointerEvent);
          element.dispatchEvent(
            new MouseEvent('mousedown', { bubbles: true, button: 0, cancelable: true }),
          );
          element.dispatchEvent(
            new MouseEvent('mouseup', { bubbles: true, button: 0, cancelable: true }),
          );
          element.dispatchEvent(
            new MouseEvent('click', { bubbles: true, button: 0, cancelable: true }),
          );
        };
        const element = Array.from(document.querySelectorAll(targetSelector)).filter(isVisible)[
          targetIndex
        ];
        if (!element) {
          throw new Error(`No visible element ${targetSelector} at index ${targetIndex}`);
        }
        dispatchSequence(element);
      },
      { index, selector: planFilterControlSelector },
    );
    return;
  }

  await page.evaluate((targetIndex) => {
    const labels = [
      ['全部', 'All'],
      ['按周期', 'By Period', 'Period'],
      ['按流量', 'By Traffic', 'Traffic'],
    ];
    const targetLabels = labels[targetIndex] ?? [];
    const textOf = (element) => (element.textContent ?? '').trim().replace(/\s+/g, ' ');
    const dispatchSequence = (element) => {
      const pointerEvent =
        typeof PointerEvent === 'function'
          ? new PointerEvent('pointerdown', {
              bubbles: true,
              button: 0,
              cancelable: true,
              pointerType: 'mouse',
            })
          : new MouseEvent('mousedown', { bubbles: true, button: 0, cancelable: true });
      element.dispatchEvent(pointerEvent);
      element.dispatchEvent(
        new MouseEvent('mousedown', { bubbles: true, button: 0, cancelable: true }),
      );
      element.dispatchEvent(
        new MouseEvent('mouseup', { bubbles: true, button: 0, cancelable: true }),
      );
      element.dispatchEvent(
        new MouseEvent('click', { bubbles: true, button: 0, cancelable: true }),
      );
    };
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const element = Array.from(
      document.querySelectorAll(
        '.ant-tabs-tab, .ant-radio-button-wrapper, .ant-segmented-item, [role="tab"], span, button',
      ),
    ).find((candidate) => isVisible(candidate) && targetLabels.includes(textOf(candidate)));
    if (!element) {
      throw new Error(`No visible plan tab for index ${targetIndex}`);
    }
    dispatchSequence(element);
  }, index);
}

export async function planTabStates(page) {
  return page.evaluate((modernSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const planTabLabels = [
      '全部',
      'All',
      '按周期',
      'By Period',
      'Period',
      '按流量',
      'By Traffic',
      'Traffic',
    ];
    const isPlanTabLabel = (element) =>
      planTabLabels.includes((element.textContent ?? '').trim().replace(/\s+/g, ' '));
    const normalizeClassName = (element) =>
      element.getAttribute('data-state') === 'active' ||
      element.getAttribute('data-state') === 'checked' ||
      element.getAttribute('aria-selected') === 'true' ||
      element.getAttribute('aria-checked') === 'true' ||
      String(element.className).split(/\s+/).includes('active') ||
      element.closest(
        '.ant-tabs-tab-active, .ant-radio-button-wrapper-checked, .ant-segmented-item-selected',
      )
        ? 'active'
        : '';
    const modernTabs = Array.from(document.querySelectorAll(modernSelector)).filter(isVisible);
    const tabs = modernTabs.length
      ? modernTabs
      : Array.from(
          document.querySelectorAll(
            '[data-testid="plan-tabs"] span, .ant-tabs-tab, .ant-radio-button-wrapper, .ant-segmented-item, [role="tab"], span, button',
          ),
        ).filter((element) => isVisible(element) && isPlanTabLabel(element));
    return tabs.map((element) => ({
      className: normalizeClassName(element),
      text: (element.textContent ?? '').trim().replace(/\s+/g, ' '),
    }));
  }, planFilterControlSelector);
}

export async function firstCommerceActionState(page, selector) {
  const state = await firstElementState(page, selector);
  return state ? { disabled: state.disabled } : null;
}

export async function commerceSummaryTexts(page, selector, limit) {
  const actionTextPattern =
    /\s*(下单|提交订单|立即订阅|结账|支付|Place Order|Subscribe Now|Checkout|Pay)$/i;
  return (await visibleTexts(page, selector, limit))
    .filter((text) => /\d/.test(text))
    .map((text) => text.trim().replace(/\s+/g, ' ').replace(actionTextPattern, ''));
}

export async function clickCouponVerifyButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const input = Array.from(
      document.querySelectorAll(
        '[data-testid="coupon-input"], .v2board-input-coupon, #cashier input[placeholder*="优惠"], #cashier input[placeholder*="Coupon"], #cashier input[placeholder*="coupon"]',
      ),
    ).find(isVisible);
    const container =
      input?.closest('.block, .input-group, [data-testid="checkout-summary"]') ??
      input?.parentElement;
    const button = container
      ? Array.from(container.querySelectorAll('button, .btn')).find(isVisible)
      : null;
    if (!button) {
      throw new Error('No visible coupon verify button');
    }
    button.click();
  });
}
