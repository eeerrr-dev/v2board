import { visibleTexts, visibleCount, visibleLinkStates } from '../dom-helpers.mjs';
import {
  normalizeProfileBlockTitles,
  normalizeDashboardConfirmContent,
  normalizeDashboardConfirmButtons,
  normalizeProfileTelegramBindBodies,
  normalizeProfileTelegramIdTexts,
  normalizeProfilePreferenceLabels,
  normalizeProfileActionButtonState,
} from '../normalizers.mjs';

export async function profileResetSubscribeState(page) {
  const title = await visibleTexts(
    page,
    '[data-testid="profile-confirm-dialog"] h2, .ant-modal-confirm-title, .ant-modal-title',
    4,
  );
  const content = await visibleTexts(
    page,
    '[data-testid="profile-confirm-dialog"] p, .ant-modal-confirm-content, .ant-modal-body',
    4,
  );
  return {
    blockTitles: normalizeProfileBlockTitles(
      await visibleTexts(
        page,
        '[data-testid="profile-card-title"], [data-testid="dashboard-card-title"], .block-title',
        12,
      ),
    ),
    buttons: await visibleTexts(
      page,
      '[data-testid="profile-confirm-dialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
      4,
    ),
    content: normalizeDashboardConfirmContent(content, title),
    modalCount: await visibleCount(
      page,
      '[data-testid="profile-confirm-dialog"], .ant-modal-confirm, .ant-modal',
    ),
    resetButtons: await visibleTexts(page, '[data-testid="profile-reset-button"], .ant-btn-danger', 4),
    resetCount: page.__visualParityUserResetSecurityCount ?? 0,
    toastTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
    title,
    warningTexts: await visibleTexts(page, '[data-testid="profile-reset-warning"], .alert-warning', 4),
  };
}

export async function profileTelegramBindState(page) {
  const modalTitles = await visibleTexts(
    page,
    '[data-testid="profile-telegram-bind-dialog"] h2, .ant-modal-title',
    4,
  );
  const modalBodies = await visibleTexts(
    page,
    '[data-testid="profile-telegram-bind-dialog"], .ant-modal .ant-modal-body',
    4,
  );
  return {
    blockTitles: normalizeProfileBlockTitles(
      await visibleTexts(page, '[data-testid="profile-card-title"], .block-title', 12),
    ),
    buttons: normalizeDashboardConfirmButtons(
      await visibleTexts(
        page,
        // Exclude the redesign's copy-command button (profile-copy-code, e.g. "/bind");
        // the legacy oracle rendered that command as inline <code>, not a button, so it
        // is compared via modalCode instead. The remaining buttons match the oracle.
        '[data-testid="profile-telegram-bind-dialog"] button:not([data-testid="profile-copy-code"]), .ant-modal-footer .ant-btn, .ant-modal .ant-btn',
        4,
      ),
    ),
    copyCommandCount: await page.evaluate(() => window.__visualParityCopyCommandCount ?? 0),
    discussionLinks: await visibleLinkStates(
      page,
      '[data-testid="profile-telegram-discuss"] a, .join_telegram_disscuss a',
    ),
    modalBodies: normalizeProfileTelegramBindBodies(modalBodies, modalTitles),
    modalCode: await visibleTexts(page, '[data-testid="profile-copy-code"], .ant-modal code', 4),
    modalCount: await visibleCount(page, '[data-testid="profile-telegram-bind-dialog"], .ant-modal'),
    modalLinks: await visibleLinkStates(page, '[data-testid="profile-telegram-bind-dialog"] a, .ant-modal a'),
    modalTitles,
    startButtons: await visibleTexts(
      page,
      '[data-testid="profile-telegram-bind"] button, .bind_telegram .btn, .bind_telegram button',
      4,
    ),
  };
}

export async function profileTelegramUnbindState(page) {
  const modalTitle = await visibleTexts(
    page,
    '[data-testid="profile-confirm-dialog"] h2, .ant-modal-confirm-title, .ant-modal-title',
    4,
  );
  const modalContent = await visibleTexts(
    page,
    '[data-testid="profile-confirm-dialog"] p, .ant-modal-confirm-content, .ant-modal-body',
    4,
  );
  return {
    blockTitles: normalizeProfileBlockTitles(
      await visibleTexts(page, '[data-testid="profile-card-title"], .block-title', 12),
    ),
    buttons: await visibleTexts(
      page,
      '[data-testid="profile-confirm-dialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
      4,
    ),
    modalContent: normalizeDashboardConfirmContent(modalContent, modalTitle),
    modalCount: await visibleCount(
      page,
      '[data-testid="profile-confirm-dialog"], .ant-modal-confirm, .ant-modal',
    ),
    modalTitle,
    telegramIdTexts: normalizeProfileTelegramIdTexts(
      await visibleTexts(
        page,
        '[data-testid="profile-telegram-unbind"] button, [data-testid="profile-telegram-id"], .unbind_telegram .block-options',
        4,
      ),
    ),
    toastTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
    unbindButtons: await visibleTexts(
      page,
      '[data-testid="profile-telegram-unbind"] button, .unbind_telegram .ant-btn, .unbind_telegram button',
      4,
    ),
    unbindCount: page.__visualParityUserUnbindTelegramCount ?? 0,
  };
}

export async function profilePreferenceSwitchesState(page) {
  const switches = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('[data-testid="profile-switch"], .ant-switch'))
      .filter(isVisible)
      .map((element) => ({
        ariaChecked: element.getAttribute('aria-checked'),
        checked: Boolean(
          element.matches('.ant-switch-checked, [aria-checked="true"], [data-state="checked"]'),
        ),
        disabled: Boolean(element.matches(':disabled, .ant-switch-disabled')),
        loading: Boolean(
          element.matches('.ant-switch-loading, [data-testid="profile-switch"][data-loading="true"]') ||
            element.getAttribute('aria-busy') === 'true' ||
            element.querySelector('.ant-switch-loading-icon'),
        ),
        role: element.getAttribute('role'),
      }));
  });
  const switchLabels = await page.evaluate(() =>
    Array.from(document.querySelectorAll('[data-testid="profile-switch"][aria-label]'))
      .map((element) => element.getAttribute('aria-label'))
      .filter((labelText) => typeof labelText === 'string' && labelText.length > 0),
  );
  const updateRequests = (page.__visualParityUserUpdateRequests ?? []).map((request) =>
    request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
  );
  return {
    blockTitles: normalizeProfileBlockTitles(
      await visibleTexts(page, '[data-testid="profile-card-title"], .block-title', 12),
    ),
    labels: normalizeProfilePreferenceLabels(
      [
        ...switchLabels,
        ...(await visibleTexts(
          page,
          '[data-testid="profile-switch"], [data-testid="profile-switch"], .text-muted, .form-group label',
          16,
        )),
      ],
    ),
    switchCount: switches.length,
    switches,
    updateRequests,
  };
}

export async function profileRedeemGiftcardState(page) {
  const domState = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const input = Array.from(document.querySelectorAll('input')).find((element) => {
      const placeholder = element.getAttribute('placeholder') ?? '';
      return isVisible(element) && /Gift Card|礼品卡/.test(placeholder);
    });
    const block = input?.closest('[data-testid="profile-gift-card"], .block') ?? null;
    const button = block
      ? (block.querySelector('[data-testid="profile-redeem-button"]') ??
          Array.from(block.querySelectorAll('button')).find(isVisible) ??
          null)
      : null;
    return {
      inputValue: input && 'value' in input ? input.value : '',
      redeemButton: button
        ? {
            className: normalizeClassName(button.className),
            disabled: Boolean(button.matches(':disabled, .ant-btn-disabled')),
            loading: Boolean(
              button.matches('.ant-btn-loading, [aria-busy="true"]') ||
                button.querySelector('.anticon-loading, .fa-spin, svg.animate-spin'),
            ),
            text: (button.textContent ?? '').trim().replace(/\s+/g, ' '),
          }
        : null,
    };
  });
  return {
    blockTitles: normalizeProfileBlockTitles(
      await visibleTexts(page, '[data-testid="profile-card-title"], .block-title', 12),
    ),
    ...domState,
    redeemButton: normalizeProfileActionButtonState(domState.redeemButton),
    redeemRequests: (page.__visualParityUserRedeemGiftcardRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    toastTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
  };
}

export async function profileChangePasswordState(page) {
  const domState = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const block =
      document.querySelector('[data-testid="profile-password-card"]') ??
      Array.from(document.querySelectorAll('.block')).find((element) => {
        const title = element.querySelector('.block-title')?.textContent ?? '';
        return isVisible(element) && /Change Password|修改密码/.test(title);
      });
    const inputs = block
      ? Array.from(block.querySelectorAll('input')).filter(isVisible).map((element) => ({
          placeholder: element.getAttribute('placeholder') ?? '',
          type: element.getAttribute('type') ?? '',
          value: 'value' in element ? element.value : '',
        }))
      : [];
    const button = block
      ? (block.querySelector('[data-testid="profile-password-save"]') ??
          Array.from(block.querySelectorAll('button')).find(isVisible) ??
          null)
      : null;
    const loginPasswordInput = Array.from(document.querySelectorAll('input[type="password"]')).find(
      isVisible,
    );
    return {
      authBoxCount: Array.from(
        document.querySelectorAll('[data-testid="auth-surface"], .v2board-auth-box'),
      ).filter(isVisible).length,
      passwordInputs: inputs,
      saveButton: button
        ? {
            className: normalizeClassName(button.className),
            disabled: Boolean(button.matches(':disabled, .ant-btn-disabled')),
            loading: Boolean(
              button.matches('.ant-btn-loading, [aria-busy="true"]') ||
                button.querySelector('.anticon-loading, .fa-spin, svg.animate-spin'),
            ),
            text: (button.textContent ?? '').trim().replace(/\s+/g, ' '),
          }
        : null,
      visibleLoginPasswordPlaceholder: loginPasswordInput?.getAttribute('placeholder') ?? '',
    };
  });
  return {
    blockTitles: normalizeProfileBlockTitles(
      await visibleTexts(
        page,
        '[data-testid="profile-card-title"], [data-testid="dashboard-card-title"], .block-title',
        12,
      ),
    ),
    changePasswordRequests: (page.__visualParityUserChangePasswordRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    hash: await page.evaluate(() => window.location.hash),
    localAuthPresent: await page.evaluate(() => Boolean(window.localStorage.getItem('authorization'))),
    toastTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
    ...domState,
    saveButton: normalizeProfileActionButtonState(domState.saveButton),
  };
}

export async function waitForProfileSwitchLoading(page, index) {
  await page
    .waitForFunction(
      ({ index: switchIndex }) => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const element = Array.from(
          document.querySelectorAll('[data-testid="profile-switch"], .ant-switch'),
        ).filter(isVisible)[switchIndex];
        return Boolean(
          element &&
            (element.matches(
              '.ant-switch-loading, .ant-switch-disabled, [data-testid="profile-switch"][data-loading="true"], :disabled',
            ) ||
              element.getAttribute('aria-busy') === 'true' ||
              element.querySelector('.ant-switch-loading-icon')),
        );
      },
      { index },
      { timeout: 5_000 },
    )
    .catch(() => undefined);
}

export async function clickProfileRedeemGiftcardButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const input = Array.from(document.querySelectorAll('input')).find((element) => {
      const placeholder = element.getAttribute('placeholder') ?? '';
      return isVisible(element) && /Gift Card|礼品卡/.test(placeholder);
    });
    const block = input?.closest('[data-testid="profile-gift-card"], .block') ?? null;
    const button = block
      ? (block.querySelector('[data-testid="profile-redeem-button"]') ??
          Array.from(block.querySelectorAll('button')).find(isVisible) ??
          null)
      : null;
    if (!button) {
      throw new Error('No visible profile giftcard redeem button');
    }
    button.click();
  });
}

export async function waitForProfileRedeemGiftcardLoading(page) {
  await page
    .waitForFunction(
      () => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const input = Array.from(document.querySelectorAll('input')).find((element) => {
          const placeholder = element.getAttribute('placeholder') ?? '';
          return isVisible(element) && /Gift Card|礼品卡/.test(placeholder);
        });
        const block = input?.closest('[data-testid="profile-gift-card"], .block') ?? null;
        const button = block
          ? (block.querySelector('[data-testid="profile-redeem-button"]') ??
              Array.from(block.querySelectorAll('button')).find(isVisible) ??
              null)
          : null;
        return Boolean(
          button &&
            (button.matches('.ant-btn-loading, :disabled, .ant-btn-disabled, [aria-busy="true"]') ||
              button.querySelector('.anticon-loading, .fa-spin, svg.animate-spin')),
        );
      },
      null,
      { timeout: 5_000 },
    )
    .catch(() => undefined);
}

export async function clickProfileChangePasswordButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const block =
      document.querySelector('[data-testid="profile-password-card"]') ??
      Array.from(document.querySelectorAll('.block')).find((element) => {
        const title = element.querySelector('.block-title')?.textContent ?? '';
        return isVisible(element) && /Change Password|修改密码/.test(title);
      });
    const button = block
      ? (block.querySelector('[data-testid="profile-password-save"]') ??
          Array.from(block.querySelectorAll('button')).find(isVisible) ??
          null)
      : null;
    if (!button) {
      throw new Error('No visible profile change password button');
    }
    button.click();
  });
}

export async function fillProfileChangePasswordInputs(page, values) {
  const inputIndexes = await page.evaluate((expectedCount) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const allInputs = Array.from(document.querySelectorAll('input'));
    const block =
      document.querySelector('[data-testid="profile-password-card"]') ??
      Array.from(document.querySelectorAll('.block')).find((element) => {
        const title = element.querySelector('.block-title')?.textContent ?? '';
        return isVisible(element) && /Change Password|修改密码/.test(title);
      });
    const inputs = block ? Array.from(block.querySelectorAll('input')).filter(isVisible) : [];
    if (inputs.length < expectedCount) {
      throw new Error(`Expected ${expectedCount} profile password inputs, got ${inputs.length}`);
    }
    return inputs.slice(0, expectedCount).map((input) => allInputs.indexOf(input));
  }, values.length);

  for (let index = 0; index < values.length; index += 1) {
    await page.locator('input').nth(inputIndexes[index]).fill(values[index]);
  }
}

export async function waitForProfileChangePasswordLoading(page) {
  await page
    .waitForFunction(
      () => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const block =
          document.querySelector('[data-testid="profile-password-card"]') ??
          Array.from(document.querySelectorAll('.block')).find((element) => {
            const title = element.querySelector('.block-title')?.textContent ?? '';
            return isVisible(element) && /Change Password|修改密码/.test(title);
          });
        const button = block
          ? (block.querySelector('[data-testid="profile-password-save"]') ??
              Array.from(block.querySelectorAll('button')).find(isVisible) ??
              null)
          : null;
        return Boolean(
          button &&
            (button.matches('.ant-btn-loading, :disabled, .ant-btn-disabled, [aria-busy="true"]') ||
              button.querySelector('.anticon-loading, .fa-spin, svg.animate-spin')),
        );
      },
      null,
      { timeout: 5_000 },
    )
    .catch(() => undefined);
}
