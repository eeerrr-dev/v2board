import {
  userAuthSurfaceSelector,
  userAuthControlSelector,
  userAuthLinkSelector,
  userAuthTitleTextSelector,
} from '../selectors.mjs';
import { visibleCount, visibleTexts } from '../dom-helpers.mjs';

export async function authPageState(page) {
  return {
    authBoxCount: await visibleCount(page, userAuthSurfaceSelector),
    buttons: await visibleTexts(page, 'button, .btn', 8),
    controls: await visibleFormControlStates(page, userAuthControlSelector),
    hash: await page.evaluate(() => window.location.hash),
    links: await visibleTexts(page, userAuthLinkSelector, 8),
    titleTexts: await visibleTexts(page, userAuthTitleTextSelector, 8),
  };
}

export async function visibleFormControlStates(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
    };
    const normalize = (value) => String(value ?? '').trim().replace(/\s+/g, ' ');
    return Array.from(document.querySelectorAll(targetSelector))
      .filter(isVisible)
      .map((element) => ({
        disabled: Boolean(element.disabled),
        options: element instanceof HTMLSelectElement
          ? Array.from(element.options).map((option) => normalize(option.textContent))
          : [],
        placeholder: element.getAttribute('placeholder') ?? '',
        tag: element.tagName.toLowerCase(),
        type: element.getAttribute('type') ?? '',
        value: 'value' in element ? element.value : '',
      }));
  }, selector);
}

export async function readSessionExpiredRedirectState(page) {
  let lastError;
  for (let attempt = 0; attempt < 5; attempt += 1) {
    await page.waitForLoadState('domcontentloaded', { timeout: 2_000 }).catch(() => undefined);
    await page.waitForLoadState('networkidle', { timeout: 2_000 }).catch(() => undefined);
    await page.waitForTimeout(150);
    try {
      return await page.evaluate(({ authSurfaceSelector, authTitleTextSelector }) => {
        const visibleText = (selector, limit) =>
          Array.from(document.querySelectorAll(selector))
            .filter((element) => {
              const rect = element.getBoundingClientRect();
              const style = window.getComputedStyle(element);
              return rect.width > 0 && rect.height > 0 && style.display !== 'none';
            })
            .slice(0, limit)
            .map((element) => (element.textContent ?? '').trim().replace(/\s+/g, ' '))
            .filter(Boolean);
        return {
          authData: window.localStorage.getItem('authorization'),
          hash: window.location.hash,
          loginBoxCount: document.querySelectorAll(authSurfaceSelector).length,
          titleTexts: visibleText(authTitleTextSelector, 4),
        };
      }, { authSurfaceSelector: userAuthSurfaceSelector, authTitleTextSelector: userAuthTitleTextSelector });
    } catch (error) {
      lastError = error;
      if (!String(error?.message ?? error).includes('Execution context was destroyed')) {
        throw error;
      }
    }
  }
  throw lastError ?? new Error('Unable to read session expired redirect state');
}

export async function readUnauthorizedHttp401NoRedirectState(page) {
  return page.evaluate((authSurfaceSelector) => {
    const visibleText = (selector, limit) =>
      Array.from(document.querySelectorAll(selector))
        .filter((element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        })
        .slice(0, limit)
        .map((element) => (element.textContent ?? '').trim().replace(/\s+/g, ' '))
        .filter(Boolean);
    return {
      authData: window.localStorage.getItem('authorization'),
      dashboardTexts: visibleText(
        '.block-title, .content-heading, .alert, .nav-main-link, .v2board-container-title, [data-testid="dashboard-page"]',
        12,
      ),
      hash: window.location.hash,
      loginBoxCount: document.querySelectorAll(authSurfaceSelector).length,
      pageContainerCount: document.querySelectorAll('#page-container').length,
    };
  }, userAuthSurfaceSelector);
}

export async function loginLanguagePersistenceState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const readCookie = (name) =>
      document.cookie.split('; ').reduce((value, item) => {
        const [key, raw] = item.split('=');
        if (key !== name || raw === undefined) return value;
        try {
          return decodeURIComponent(raw);
        } catch {
          return value;
        }
      }, '');

    // titleText is intentionally not captured: the redesign turns the brand link into a semantic
    // <h1>, and the operator brand is constant across locales, so it carries no language-persistence
    // signal. Releasing it keeps this interaction gating the locale state (cookie/storage/trigger),
    // not the heading markup the redesign legitimately changed.
    return {
      cookieI18n: readCookie('i18n'),
      gLang: window.g_lang ?? '',
      storedLocale: window.localStorage.getItem('umi_locale') ?? '',
      triggerText: normalize(
        document.querySelector('.v2board-auth-language-trigger, .v2board-login-i18n-btn')
          ?.textContent,
      ),
    };
  });
}
