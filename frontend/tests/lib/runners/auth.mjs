import {
  adminAuthPageState,
  loginLanguagePersistenceState,
  readAdminSessionExpiredRedirectState,
  readSessionExpiredRedirectState,
  readUnauthorizedHttp401NoRedirectState,
  visibleFormControlStates,
} from '../state-readers/auth.mjs';
import { normalizeRedesignedAuthPageState, withoutDroppedLocale } from '../normalizers.mjs';
import {
  fillFirstVisible,
  clickFirstVisibleWithPointer,
  firstInputValue,
  visibleTexts,
  clickFirstVisibleTextStable,
  fillVisibleAt,
  waitForVisibleElementCountAtLeast,
  visibleCount,
} from '../dom-helpers.mjs';
import {
  adminAuthForgotSelector,
  adminAuthIdentifierSelector,
  adminAuthPasswordSelector,
  adminAuthSurfaceSelector,
  adminForgotDialogSelector,
  languageMenuItemSelector,
  userAuthControlSelector,
  userAuthSurfaceSelector,
} from '../selectors.mjs';

export async function runRedesignedLoginPageStateInteraction(page) {
  return normalizeRedesignedAuthPageState(page);
}

export async function runLoginFormLanguageInteraction(page) {
  await fillFirstVisible(
    page,
    'input[type="text"], input:not([type]), input[type="email"]',
    'visual@example.com',
  );
  await fillFirstVisible(page, 'input[type="password"]', 'secret123');
  await clickFirstVisibleWithPointer(
    page,
    '[data-testid="auth-language-trigger"], .ant-dropdown-trigger',
  );
  await page.waitForTimeout(150);
  return {
    email: await firstInputValue(
      page,
      'input[type="text"], input:not([type]), input[type="email"]',
    ),
    languageMenuItems: withoutDroppedLocale(await visibleTexts(page, languageMenuItemSelector, 8)),
    password: await firstInputValue(page, 'input[type="password"]'),
  };
}

export async function runLoginLanguagePersistenceInteraction(page) {
  const before = await loginLanguagePersistenceState(page);
  await clickFirstVisibleWithPointer(
    page,
    '[data-testid="auth-language-trigger"], .ant-dropdown-trigger',
  );
  await page.waitForTimeout(150);
  const menuItems = withoutDroppedLocale(await visibleTexts(page, languageMenuItemSelector, 8));
  const navigation = page
    .waitForNavigation({ waitUntil: 'domcontentloaded', timeout: 3_000 })
    .catch(() => undefined);
  await clickFirstVisibleTextStable(page, languageMenuItemSelector, ['English']);
  await navigation;
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await page.waitForTimeout(500);
  const afterSelect = await loginLanguagePersistenceState(page);
  await page.reload({ waitUntil: 'domcontentloaded', timeout: 10_000 });
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await page.waitForTimeout(500);
  const afterReload = await loginLanguagePersistenceState(page);

  return {
    afterReload,
    afterSelect,
    before,
    menuItems,
  };
}

export async function runAuthPageStateInteraction(page) {
  return adminAuthPageState(page);
}

export async function runRegisterFormStateInteraction(page) {
  await fillVisibleAt(
    page,
    'input[type="text"], input:not([type]), input[type="email"]',
    0,
    'parity-user',
  );
  await fillVisibleAt(page, 'input[type="password"]', 0, 'secret123');
  await fillVisibleAt(page, 'input[type="password"]', 1, 'secret123');
  return normalizeRedesignedAuthPageState(page);
}

// §10.3 boot translator (docs/api-dialect.md Appendix A §W1): the scenario
// enters the path-routed source world through the legacy
// `/#/register?code=INVITE2026` URL (`legacyHashEntry`). With
// `legacy_hash_redirect_enable` ON the boot must replace it with the history
// URL before the router mounts — canonical path, query string preserved, hash
// consumed — and the register form must still consume the invite code.
// Source-only: the frozen oracle is hash-routed by design and never
// translates.
export async function runUserRegisterLegacyHashEntryInteraction(page) {
  return {
    authBoxCount: await visibleCount(page, userAuthSurfaceSelector),
    controls: await visibleFormControlStates(page, userAuthControlSelector),
    ...(await page.evaluate(() => ({
      historyPath: window.location.pathname + window.location.search,
      locationHash: window.location.hash,
      route: window.__parityReadSpaRoute(),
    }))),
  };
}

export async function runForgetFormStateInteraction(page) {
  await fillVisibleAt(
    page,
    'input[type="text"], input:not([type]), input[type="email"]',
    0,
    'visual@example.com',
  );
  await fillVisibleAt(
    page,
    'input[type="text"], input:not([type]), input[type="email"]',
    1,
    '123456',
  );
  await fillVisibleAt(page, 'input[type="password"]', 0, 'secret123');
  await fillVisibleAt(page, 'input[type="password"]', 1, 'secret123');
  return normalizeRedesignedAuthPageState(page);
}

export async function runAdminLoginFormStateInteraction(page) {
  await fillFirstVisible(page, adminAuthIdentifierSelector, 'admin@example.com');
  await fillFirstVisible(page, adminAuthPasswordSelector, '12345678');
  const filled = await adminAuthPageState(page);
  await clickFirstVisibleWithPointer(page, adminAuthForgotSelector);
  await waitForVisibleElementCountAtLeast(page, adminForgotDialogSelector, 1);
  return {
    filled,
    forgotModal: {
      buttons: await visibleTexts(
        page,
        '[role="dialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
        4,
      ),
      content: await visibleTexts(
        page,
        '[role="dialog"] [data-slot="dialog-description"], [role="dialog"] code, .ant-modal-confirm-content, .ant-modal-body',
        4,
      ),
      modalCount: await visibleCount(page, adminForgotDialogSelector),
    },
  };
}

export async function runAdminSystemQueueStateInteraction(page) {
  await page.waitForTimeout(150);
  return {
    hash: await page.evaluate(() => window.__parityReadSpaRoute()),
    overview: await visibleTexts(
      page,
      '[data-testid="queue-page"] [data-slot="card-title"], [data-testid="queue-page"] h2, .block-title, .font-size-h3',
      12,
    ),
    rows: await visibleTexts(
      page,
      '[data-testid="queue-workload-table"] tbody tr, .ant-table-tbody tr',
      12,
    ),
    tableHeaders: await visibleTexts(
      page,
      '[data-testid="queue-workload-table"] thead th, .ant-table-thead th',
      8,
    ),
  };
}

export async function runSessionExpiredRedirectInteraction(page) {
  await page.waitForFunction(
    (authSurfaceSelector) =>
      window.__parityReadSpaRoute().includes('/login') &&
      Boolean(document.querySelector(authSurfaceSelector)),
    userAuthSurfaceSelector,
    { timeout: 5_000 },
  );
  return readSessionExpiredRedirectState(page);
}

export async function runAdminSessionExpiredRedirectInteraction(page) {
  await page.waitForFunction(
    (authSurfaceSelector) =>
      window.__parityReadSpaRoute().includes('/login') &&
      Boolean(document.querySelector(authSurfaceSelector)),
    adminAuthSurfaceSelector,
    { timeout: 5_000 },
  );
  return readAdminSessionExpiredRedirectState(page);
}

export async function runUnauthorizedHttp401NoRedirectInteraction(page) {
  await page.waitForTimeout(500);
  return readUnauthorizedHttp401NoRedirectState(page);
}
