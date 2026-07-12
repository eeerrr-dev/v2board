import {
  clickFirstVisible,
  clickFirstVisibleText,
  clickVisibleAt,
  visibleCount,
  visibleTexts,
  waitForPageProperty,
  waitForPagePropertyAtLeast,
  waitForVisibleElementsHidden,
  waitForVisibleText,
} from '../dom-helpers.mjs';
import { dashboardResetPackageTradeNo } from '../fixture-data.mjs';
import { normalizeDashboardOrderInfo, withoutDroppedLocale } from '../normalizers.mjs';
import {
  waitForFixedColumnLayout,
  waitForFontsBeforeCapture,
  waitForMountedContent,
} from '../page-prep.mjs';
import {
  adminDashboardShortcutState,
  clickHeaderAvatarTrigger,
  headerAvatarDropdownState,
  waitForHeaderAvatarDropdown,
} from '../state-readers/admin.mjs';
import {
  clickDashboardSubscribeShortcut,
  dashboardAlertLinksState,
  dashboardNewPeriodConfirmState,
  dashboardNoticeCarouselState,
  dashboardResetPackageConfirmState,
  dashboardSubscribeImportLinksState,
  dashboardSubscribeState,
  languageDropdownPlacementState,
} from '../state-readers/dashboard.mjs';
import {
  clickDarkModeButton,
  darkModePersistenceState,
  waitForCurrentDarkModeRuntime,
  waitForStableDarkModeStyleSnapshot,
} from '../state-readers/darkmode.mjs';

export async function runDashboardHeaderLanguageDropdownInteraction(page) {
  // The redesigned shell nests the language switcher inside the sidebar-footer
  // account menu (avatar → Language submenu) while the oracle keeps its header
  // .fa-language ant-dropdown; walk whichever chrome the page renders. The
  // gated outcome — one dropdown listing the enabled locales — stays shared.
  const legacyClicked = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const trigger = Array.from(
      document.querySelectorAll('#page-header button, #page-header .ant-dropdown-trigger'),
    ).find((element) => isVisible(element) && element.querySelector('.fa-language'));
    if (!(trigger instanceof HTMLElement)) return false;
    trigger.click();
    return true;
  });
  if (!legacyClicked) {
    // Mobile keeps the account card inside the closed nav sheet; open it first.
    const avatarVisible = await page.evaluate(() => {
      const element = document.querySelector('[data-testid="app-avatar-trigger"]');
      if (!element) return false;
      const rect = element.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0;
    });
    if (!avatarVisible) {
      await page.click('#page-header [data-sidebar="trigger"]');
      await page.waitForSelector('[data-testid="app-avatar-trigger"]', {
        state: 'visible',
        timeout: 5_000,
      });
    }
    const opened = await page.evaluate(() => {
      const trigger = document.querySelector('[data-testid="app-avatar-trigger"]');
      if (!(trigger instanceof HTMLElement)) return false;
      trigger.dispatchEvent(
        new PointerEvent('pointerdown', { bubbles: true, button: 0, ctrlKey: false }),
      );
      return true;
    });
    if (!opened) throw new Error('dashboard account-menu trigger was not visible');
    await page.waitForSelector('[data-testid="app-language-trigger"]', {
      state: 'visible',
      timeout: 5_000,
    });
    await page.click('[data-testid="app-language-trigger"]');
  }
  await waitForVisibleText(
    page,
    '[data-testid="app-language-menu"] [role="menuitem"], [data-testid="app-language-menu"] [role="menuitemradio"], .ant-dropdown-menu-item',
    'English',
  );
  await page.waitForTimeout(150);
  // Same conscious fa-IR drop as the login language menu: the product ships 6 LTR locales while the
  // frozen oracle still lists فارسی. Normalize that one retired locale out of the captured list so
  // this redesigned dashboard shell keeps gating the i18n locale list without re-pinning a locale
  // the product no longer ships.
  const state = await languageDropdownPlacementState(page);
  return { ...state, items: withoutDroppedLocale(state.items) };
}

export async function runDarkModePersistenceInteraction(page) {
  const diagnostics = page.__visualParityDiagnostics ?? [];
  const before = await darkModePersistenceState(page);
  await clickDarkModeButton(page);
  await waitForCurrentDarkModeRuntime(page, diagnostics);
  const afterEnable = {
    ...(await darkModePersistenceState(page)),
    styleSnapshot: await waitForStableDarkModeStyleSnapshot(page, diagnostics),
  };
  await page.reload({ waitUntil: 'domcontentloaded', timeout: 10_000 });
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await waitForCurrentDarkModeRuntime(page, diagnostics);
  await waitForMountedContent(page, diagnostics);
  await waitForFontsBeforeCapture(page, diagnostics);
  await waitForFixedColumnLayout(page);
  const afterReload = {
    ...(await darkModePersistenceState(page)),
    styleSnapshot: await waitForStableDarkModeStyleSnapshot(page, diagnostics),
  };

  return {
    afterEnable,
    afterReload,
    before,
  };
}

export async function runDashboardSubscribeDrawerInteraction(page) {
  await page.evaluate(() => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (value) => {
          window.__visualParityClipboardWrites = [
            ...(window.__visualParityClipboardWrites ?? []),
            String(value),
          ];
        },
      },
    });
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: (command) => command === 'copy',
    });
  });

  const before = await dashboardSubscribeState(page);
  await clickDashboardSubscribeShortcut(page);
  await page.waitForSelector('[data-testid="dashboard-subscribe-menu"], .oneClickSubscribe___2t9Xg', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(350);
  const opened = await dashboardSubscribeState(page);

  await clickFirstVisible(
    page,
    '[data-testid="dashboard-subscribe-copy"], .oneClickSubscribe___2t9Xg .subsrcibe-for-link',
  );
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const copied = await dashboardSubscribeState(page);

  await clickFirstVisible(
    page,
    '[data-testid="dashboard-subscribe-qrcode"], .oneClickSubscribe___2t9Xg .subscribe-for-qrcode',
  );
  // Redesigned source renders the QR as an <svg> (qrcode.react QRCodeSVG); the
  // legacy oracle renders a <canvas>. Accept either, scoped to the QR wrapper so
  // the dialog's lucide close-button <svg> is not mistaken for the QR.
  await page.waitForSelector(
    '[data-testid="dashboard-subscribe-qrcode-image"] svg, [data-testid="dashboard-subscribe-qrcode-image"] canvas, .ant-modal canvas',
    {
      state: 'visible',
      timeout: 5_000,
    },
  );
  await page.waitForTimeout(100);
  const qr = await dashboardSubscribeState(page);

  return { before, copied, opened, qr };
}

export async function runDashboardSubscribeImportLinksInteraction(page) {
  return await runDashboardSubscribeImportLinksInteractionFor(['Hiddify', 'Sing-box'])(page);
}

export function runDashboardSubscribeImportLinksInteractionFor(expectedTargets) {
  return async (page) => {
    const before = await dashboardSubscribeImportLinksState(page);
    await clickDashboardSubscribeShortcut(page);
    await page.waitForSelector('[data-testid="dashboard-subscribe-menu"], .oneClickSubscribe___2t9Xg', {
      state: 'visible',
      timeout: 5_000,
    });
    await page.waitForTimeout(350);
    const opened = await dashboardSubscribeImportLinksState(page);

    return { before, expectedTargets, opened };
  };
}

export async function runDashboardNoticeCarouselInteraction(page) {
  const before = await dashboardNoticeCarouselState(page);
  await clickVisibleAt(
    page,
    '[data-testid="dashboard-notice-dots"] [data-testid="dashboard-notice-dot"], .slick-dots li button',
    1,
  );
  await page.waitForTimeout(600);
  const afterDot = await dashboardNoticeCarouselState(page);

  await clickFirstVisible(
    page,
    '[data-testid="dashboard-notice-slide"][data-active="true"] [data-testid="dashboard-notice-card"], .slick-slide.slick-active [data-testid="dashboard-notice-card"], .slick-slide.slick-active a.block',
  );
  await page.waitForSelector('[data-testid="dashboard-dialog"], .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await dashboardNoticeCarouselState(page);

  await page.keyboard.press('Escape');
  await waitForVisibleElementsHidden(page, '[data-testid="dashboard-dialog"], .ant-modal');
  const closed = await dashboardNoticeCarouselState(page);

  return { afterDot, before, closed, opened };
}

export async function runDashboardResetPackageConfirmInteraction(page) {
  const initialOrderSaveCount = page.__visualParityUserOrderSaveCount ?? 0;
  const before = await dashboardResetPackageConfirmState(page);
  await clickFirstVisibleText(page, 'a, button', ['购买流量重置包']);
  await page.waitForSelector('[data-testid="dashboard-dialog"], .ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await dashboardResetPackageConfirmState(page);

  await clickFirstVisible(
    page,
    '[data-testid="dashboard-dialog"] [data-testid="dashboard-confirm-primary"], [data-testid="dashboard-dialog"] .ant-btn-primary, .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '[data-testid="dashboard-dialog"], .ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderSaveCount',
    initialOrderSaveCount + 1,
  );
  await page.waitForFunction(
    (tradeNo) => window.location.hash.includes(`/order/${tradeNo}`),
    dashboardResetPackageTradeNo,
    { timeout: 5_000 },
  );
  await page.waitForFunction(
    (tradeNo) => document.body.textContent?.includes(tradeNo),
    dashboardResetPackageTradeNo,
    { timeout: 10_000 },
  );
  await page.waitForTimeout(500);
  const confirmed = await dashboardResetPackageConfirmState(page);

  return {
    before,
    confirmed,
    hash: await page.evaluate(() => window.location.hash),
    opened,
    orderInfo: normalizeDashboardOrderInfo(await visibleTexts(page, '[data-testid="order-info"]', 6)),
    orderSaveRequests: (page.__visualParityUserOrderSaveRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
  };
}

export async function runDashboardNewPeriodConfirmInteraction(page) {
  const initialNewPeriodCount = page.__visualParityUserNewPeriodCount ?? 0;
  const before = await dashboardNewPeriodConfirmState(page);

  await clickFirstVisibleText(page, 'a, button', ['提前开启流量周期']);
  await page.waitForSelector('[data-testid="dashboard-dialog"], .ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await dashboardNewPeriodConfirmState(page);

  await clickFirstVisible(
    page,
    '[data-testid="dashboard-dialog"] [data-testid="dashboard-confirm-primary"], [data-testid="dashboard-dialog"] .ant-btn-primary, .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '[data-testid="dashboard-dialog"], .ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserNewPeriodCount',
    initialNewPeriodCount + 1,
  );
  const confirmed = await dashboardNewPeriodConfirmState(page);

  return {
    before,
    confirmed,
    hash: await page.evaluate(() => window.location.hash),
    newPeriodRequests: (page.__visualParityUserNewPeriodRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    opened,
  };
}

export async function runDashboardAlertLinksInteraction(page) {
  const before = await dashboardAlertLinksState(page);

  await clickVisibleAt(
    page,
    '[data-testid="dashboard-alert"][data-alert-kind="danger"] [data-testid="dashboard-alert-link"], .alert-danger .alert-link',
    0,
  );
  await page.waitForFunction(() => window.location.hash.includes('/order'), null, {
    timeout: 5_000,
  });
  await page.waitForSelector('[data-testid="orders-table"], .ant-table-thead, .am-list-body', {
    state: 'visible',
    timeout: 10_000,
  });
  await page.waitForTimeout(150);
  const order = await dashboardAlertLinksState(page);

  await page.evaluate(() => {
    window.location.hash = '#/dashboard';
  });
  await page.waitForSelector(
    '[data-testid="dashboard-alert-link"], .alert .alert-link',
    { state: 'visible', timeout: 10_000 },
  );
  await page.waitForTimeout(300);
  const reset = await dashboardAlertLinksState(page);

  await clickVisibleAt(
    page,
    '[data-testid="dashboard-alert"][data-alert-kind="warning"] [data-testid="dashboard-alert-link"], .alert-warning .alert-link',
    0,
  );
  await page.waitForFunction(() => window.location.hash.includes('/ticket'), null, {
    timeout: 5_000,
  });
  await page.waitForSelector('[data-testid="ticket-table"], .ant-table-thead, .am-list-body', {
    state: 'visible',
    timeout: 10_000,
  });
  await page.waitForTimeout(150);
  const ticket = await dashboardAlertLinksState(page);

  return { before, order, reset, ticket };
}

export async function runAdminDashboardCommissionShortcutInteraction(page) {
  const before = await adminDashboardShortcutState(page);
  // Redesign exposes the commission alert's action by testid; the oracle renders it as the
  // second `.alert-danger .alert-link`. Drive whichever this build provides.
  if ((await visibleCount(page, '[data-testid="dashboard-commission-action"]')) > 0) {
    await clickFirstVisible(page, '[data-testid="dashboard-commission-action"]');
  } else {
    await clickVisibleAt(page, '.alert-danger .alert-link', 1);
  }
  await page.waitForFunction(() => window.location.hash.includes('/order'), null, {
    timeout: 5_000,
  });
  await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
  await page.waitForTimeout(150);
  const after = await adminDashboardShortcutState(page);

  return { after, before };
}

export async function runAdminDashboardAvatarDropdownInteraction(page) {
  const before = await headerAvatarDropdownState(page);
  await clickHeaderAvatarTrigger(page);
  await waitForHeaderAvatarDropdown(page);
  await page.waitForTimeout(150);
  const opened = await headerAvatarDropdownState(page);
  return { before, opened };
}
