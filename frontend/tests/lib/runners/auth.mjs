import {
  authPageState,
  loginLanguagePersistenceState,
  readSessionExpiredRedirectState,
  readUnauthorizedHttp401NoRedirectState,
} from '../state-readers/auth.mjs';
import { normalizeRedesignedAuthPageState, withoutDroppedLocale } from '../normalizers.mjs';
import {
  fillFirstVisible,
  clickFirstVisibleWithPointer,
  firstInputValue,
  visibleTexts,
  clickFirstVisibleText,
  fillVisibleAt,
  waitForVisibleElementCountAtLeast,
  visibleCount,
} from '../dom-helpers.mjs';
import { userAuthSurfaceSelector, languageMenuItemSelector } from '../selectors.mjs';

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
  await clickFirstVisibleWithPointer(page, '.v2board-auth-language-trigger, .ant-dropdown-trigger');
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
  await clickFirstVisibleWithPointer(page, '.v2board-auth-language-trigger, .ant-dropdown-trigger');
  await page.waitForTimeout(150);
  const menuItems = withoutDroppedLocale(await visibleTexts(page, languageMenuItemSelector, 8));
  const navigation = page.waitForNavigation({ waitUntil: 'domcontentloaded', timeout: 3_000 }).catch(
    () => undefined,
  );
  await clickFirstVisibleText(page, languageMenuItemSelector, ['English']);
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
  return authPageState(page);
}

export async function runRegisterFormStateInteraction(page) {
  await fillVisibleAt(page, 'input[type="text"], input:not([type]), input[type="email"]', 0, 'parity-user');
  await fillVisibleAt(page, 'input[type="password"]', 0, 'secret123');
  await fillVisibleAt(page, 'input[type="password"]', 1, 'secret123');
  return normalizeRedesignedAuthPageState(page);
}

export async function runForgetFormStateInteraction(page) {
  await fillVisibleAt(page, 'input[type="text"], input:not([type]), input[type="email"]', 0, 'visual@example.com');
  await fillVisibleAt(page, 'input[type="text"], input:not([type]), input[type="email"]', 1, '123456');
  await fillVisibleAt(page, 'input[type="password"]', 0, 'secret123');
  await fillVisibleAt(page, 'input[type="password"]', 1, 'secret123');
  return normalizeRedesignedAuthPageState(page);
}

export async function runAdminLoginFormStateInteraction(page) {
  await fillVisibleAt(page, 'input[type="text"], input:not([type]), input[type="email"]', 0, 'admin@local');
  await fillVisibleAt(page, 'input[type="password"]', 0, '12345678');
  const filled = await authPageState(page);
  // Redesign renders the forgot affordance as a `<button>` opening a Dialog; oracle uses an
  // `<a>` opening an antd modal. Union both so one run drives either build.
  await clickFirstVisibleText(page, 'a, button', ['忘记密码']);
  await waitForVisibleElementCountAtLeast(
    page,
    '[role="dialog"], .ant-modal-confirm, .ant-modal',
    1,
  );
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
      modalCount: await visibleCount(
        page,
        '[data-slot="dialog-content"], .ant-modal-confirm, .ant-modal',
      ),
      title: await visibleTexts(
        page,
        '[role="dialog"] [data-slot="dialog-title"], .ant-modal-confirm-title, .ant-modal-title',
        2,
      ),
    },
  };
}

export async function runAdminSystemQueueStateInteraction(page) {
  await page.waitForTimeout(150);
  return {
    hash: await page.evaluate(() => window.location.hash),
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
      window.location.hash.includes('/login') &&
      Boolean(document.querySelector(authSurfaceSelector)),
    userAuthSurfaceSelector,
    { timeout: 5_000 },
  );
  return readSessionExpiredRedirectState(page);
}

export async function runUnauthorizedHttp401NoRedirectInteraction(page) {
  await page.waitForTimeout(500);
  return readUnauthorizedHttp401NoRedirectState(page);
}
