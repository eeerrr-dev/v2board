import {
  clickFirstVisible,
  fillFirstVisible,
  firstInputValue,
  visibleTexts,
  visibleCount,
  waitForPagePropertyAtLeast,
  clickFirstVisibleText,
  waitForVisibleElementsHidden,
  clickVisibleAt,
} from '../dom-helpers.mjs';
import { normalizeDashboardOrderInfo } from '../normalizers.mjs';
import { profileDepositTradeNo } from '../fixture-data.mjs';
import {
  profileResetSubscribeState,
  profileTelegramBindState,
  profileTelegramUnbindState,
  profilePreferenceSwitchesState,
  waitForProfileSwitchLoading,
  profileRedeemGiftcardState,
  clickProfileRedeemGiftcardButton,
  waitForProfileRedeemGiftcardLoading,
  profileChangePasswordState,
  fillProfileChangePasswordInputs,
  clickProfileChangePasswordButton,
  waitForProfileChangePasswordLoading,
} from '../state-readers/profile.mjs';

export async function runProfileDepositModalInteraction(page) {
  await clickFirstVisible(page, '[data-testid="profile-recharge"], .ant-btn-primary');
  await page.waitForSelector('[data-testid="profile-deposit-dialog"], .ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillFirstVisible(
    page,
    '[data-testid="profile-deposit-input"], .ant-modal-confirm input, .ant-modal input',
    '12.34',
  );
  await page.waitForTimeout(100);
  const filled = {
    amount: await firstInputValue(
      page,
      '[data-testid="profile-deposit-input"], .ant-modal-confirm input, .ant-modal input',
    ),
    buttons: await visibleTexts(
      page,
      '[data-testid="profile-deposit-dialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
      4,
    ),
    modalCount: await visibleCount(
      page,
      '[data-testid="profile-deposit-dialog"], .ant-modal-confirm, .ant-modal',
    ),
  };

  await clickFirstVisible(
    page,
    '[data-testid="profile-deposit-confirm"], .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForPagePropertyAtLeast(page, '__visualParityUserOrderSaveCount', 1);
  await page.waitForFunction(
    (tradeNo) => window.__parityReadSpaRoute().includes(`/order/${tradeNo}`),
    profileDepositTradeNo,
    { timeout: 5_000 },
  );
  await page.waitForFunction(
    (tradeNo) => document.body.textContent?.includes(tradeNo),
    profileDepositTradeNo,
    { timeout: 10_000 },
  );
  await page.waitForTimeout(500);

  return {
    filled,
    hash: await page.evaluate(() => window.__parityReadSpaRoute()),
    orderInfo: normalizeDashboardOrderInfo(
      await visibleTexts(page, '[data-testid="order-info"], .v2board-order-summary', 6),
    ),
    orderSaveRequests: (page.__visualParityUserOrderSaveRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
  };
}

export async function runProfileResetSubscribeConfirmInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const initialSubscribeFetchCount = page.__visualParityUserSubscribeFetchCount ?? 0;
  const before = await profileResetSubscribeState(page);
  await clickFirstVisibleText(page, 'a, button', ['重置', 'Reset']);
  await page.waitForSelector('[data-testid="profile-confirm-dialog"], .ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await profileResetSubscribeState(page);

  await clickFirstVisible(
    page,
    '[data-testid="profile-confirm-primary"], .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(
    page,
    '[data-testid="profile-confirm-dialog"], .ant-modal-confirm, .ant-modal',
  );
  await waitForPagePropertyAtLeast(page, '__visualParityUserResetSecurityCount', 1);
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const confirmed = await profileResetSubscribeState(page);

  return {
    before,
    confirmed,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    opened,
    subscribeFetchDelta:
      (page.__visualParityUserSubscribeFetchCount ?? 0) - initialSubscribeFetchCount,
  };
}

export async function runProfileTelegramBindModalInteraction(page) {
  await page.evaluate(() => {
    window.__visualParityCopyCommandCount = 0;
    const recordCopy = () => {
      window.__visualParityCopyCommandCount += 1;
    };
    const clipboard = navigator.clipboard ?? {};
    Object.defineProperty(clipboard, 'writeText', {
      configurable: true,
      value: async () => recordCopy(),
    });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: clipboard,
    });
    // The frozen oracle still calls execCommand. Production source is guarded
    // against that deprecated API and exercises navigator.clipboard above.
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: (command) => {
        if (command === 'copy') recordCopy();
        return command === 'copy';
      },
    });
  });

  const before = await profileTelegramBindState(page);
  await clickFirstVisibleText(
    page,
    '[data-testid="profile-telegram-bind"] button, .bind_telegram a, .bind_telegram button',
    ['立即开始', 'Start Now'],
  );
  await page.waitForSelector('[data-testid="profile-telegram-bind-dialog"], .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      document
        .querySelector('[data-testid="profile-telegram-bind-dialog"], .ant-modal')
        ?.textContent?.includes('@legacy_bot'),
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(150);
  const opened = await profileTelegramBindState(page);

  await clickFirstVisible(page, '[data-testid="profile-copy-code"], .ant-modal code');
  await page.waitForFunction(() => (window.__visualParityCopyCommandCount ?? 0) > 0, null, {
    timeout: 5_000,
  });
  const copied = await profileTelegramBindState(page);

  await clickFirstVisible(
    page,
    '[data-testid="profile-telegram-bind-confirm"], .ant-modal-footer .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '[data-testid="profile-telegram-bind-dialog"], .ant-modal');
  const closed = await profileTelegramBindState(page);

  return { before, closed, copied, opened };
}

export async function runProfileTelegramUnbindConfirmInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await profileTelegramUnbindState(page);

  await clickFirstVisibleText(
    page,
    '[data-testid="profile-telegram-unbind"] button, .unbind_telegram button, .unbind_telegram .ant-btn',
    ['解除绑定'],
  );
  await page.waitForSelector('[data-testid="profile-confirm-dialog"], .ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await profileTelegramUnbindState(page);

  await clickFirstVisible(
    page,
    '[data-testid="profile-confirm-primary"], .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(
    page,
    '[data-testid="profile-confirm-dialog"], .ant-modal-confirm, .ant-modal',
  );
  await waitForPagePropertyAtLeast(page, '__visualParityUserUnbindTelegramCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserInfoFetchCount',
    initialInfoFetchCount + 1,
  );
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const confirmed = await profileTelegramUnbindState(page);

  return {
    before,
    confirmed,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    opened,
  };
}

export async function runProfilePreferenceSwitchesInteraction(page) {
  const preferenceKeys = ['auto_renewal', 'remind_expire', 'remind_traffic'];
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await profilePreferenceSwitchesState(page);
  const toggles = [];

  for (let index = 0; index < preferenceKeys.length; index += 1) {
    const infoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
    const updateResponse = page.waitForResponse(
      (response) => {
        const url = new URL(response.url());
        return (
          url.pathname === '/api/v1/user/update' && response.request().method() === 'POST'
        );
      },
      { timeout: 5_000 },
    );

    await clickVisibleAt(page, '[data-testid="profile-switch"], .ant-switch', index);
    await waitForProfileSwitchLoading(page, index);
    const loading = await profilePreferenceSwitchesState(page);

    await updateResponse;
    await waitForPagePropertyAtLeast(
      page,
      '__visualParityUserInfoFetchCount',
      infoFetchCount + 1,
    );
    await page.waitForTimeout(100);

    const after = await profilePreferenceSwitchesState(page);
    toggles.push({
      afterSwitch: after.switches[index],
      field: preferenceKeys[index],
      loadingSwitch: loading.switches[index],
      updateRequestCount: after.updateRequests.length,
    });
  }

  const after = await profilePreferenceSwitchesState(page);
  return {
    after,
    before,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    toggles,
  };
}

export async function runProfileRedeemGiftcardInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await profileRedeemGiftcardState(page);

  await page
    .locator('input[placeholder*="Gift Card"], input[placeholder*="礼品卡"]')
    .first()
    .fill('CARD-123');
  await page.waitForTimeout(100);
  const filled = await profileRedeemGiftcardState(page);

  await clickProfileRedeemGiftcardButton(page);
  await waitForProfileRedeemGiftcardLoading(page);
  const loading = await profileRedeemGiftcardState(page);

  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserInfoFetchCount',
    initialInfoFetchCount + 1,
  );
  await page.waitForTimeout(100);
  const after = await profileRedeemGiftcardState(page);

  return {
    after,
    before,
    filled,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    loading,
  };
}

export async function runProfileRedeemGiftcardFailureInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const initialRedeemCount = page.__visualParityUserRedeemGiftcardCount ?? 0;
  const before = await profileRedeemGiftcardState(page);

  await page
    .locator('input[placeholder*="Gift Card"], input[placeholder*="礼品卡"]')
    .first()
    .fill('CARD-FAIL');
  await page.waitForTimeout(100);
  const filled = await profileRedeemGiftcardState(page);

  await clickProfileRedeemGiftcardButton(page);
  await waitForProfileRedeemGiftcardLoading(page);
  const loading = await profileRedeemGiftcardState(page);

  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserRedeemGiftcardCount',
    initialRedeemCount + 1,
  );
  await page.waitForTimeout(350);
  const after = await profileRedeemGiftcardState(page);

  return {
    after,
    before,
    filled,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    loading,
  };
}

export async function runProfileChangePasswordSuccessInteraction(page) {
  const before = await profileChangePasswordState(page);

  await fillProfileChangePasswordInputs(page, ['old-password', 'new-password', 'new-password']);
  await page.waitForTimeout(100);
  const filled = await profileChangePasswordState(page);

  await clickProfileChangePasswordButton(page);
  await waitForProfileChangePasswordLoading(page);
  const loading = await profileChangePasswordState(page);

  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () => window.__parityReadSpaRoute().includes('/login') || window.__parityReadSpaRoute().includes('/dashboard'),
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(300);
  const after = await profileChangePasswordState(page);

  return { after, before, filled, loading };
}
