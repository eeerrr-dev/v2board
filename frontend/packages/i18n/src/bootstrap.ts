import { NAVIGATOR_LOCALES, isSupportedLocale, type SupportedLocale } from './locale-registry';

export function normalizeSupportedLocale(
  locale: string | null | undefined,
  fallback: SupportedLocale,
): SupportedLocale {
  const safeFallback = isSupportedLocale(fallback) ? fallback : 'zh-CN';
  return resolveSupportedLocale(locale) ?? safeFallback;
}

/** Resolve exact codes plus established language/underscore aliases without
 * ever manufacturing an unsupported locale. */
export function resolveSupportedLocale(
  locale: string | null | undefined,
): SupportedLocale | undefined {
  const raw = locale?.trim().split('@', 1)[0]?.replaceAll('_', '-');
  if (!raw) return undefined;
  if (isSupportedLocale(raw)) return raw;

  let parsed: Intl.Locale;
  try {
    parsed = new Intl.Locale(raw);
  } catch {
    return undefined;
  }
  const language = parsed.language.toLowerCase();
  const region = parsed.region?.toUpperCase();
  const script = parsed.script?.toLowerCase();
  if (language === 'zh') {
    return script === 'hant' || region === 'TW' || region === 'HK' || region === 'MO'
      ? 'zh-TW'
      : 'zh-CN';
  }
  return NAVIGATOR_LOCALES[language];
}

/** Resolve the browser's ordered language preferences, not only its first
 * entry. This lets an unsupported primary language fall through to a supported
 * secondary preference before the product fallback is used. */
export function resolveNavigatorLocale(
  navigator: Pick<Navigator, 'language' | 'languages'>,
): SupportedLocale | undefined {
  const candidates = [...navigator.languages, navigator.language];
  for (const candidate of candidates) {
    const locale = resolveSupportedLocale(candidate);
    if (locale) return locale;
  }
  return undefined;
}

/**
 * One-time migration read at i18n bootstrap (docs/api-dialect.md §11):
 * `v2board_locale` → `umi_locale` → legacy `i18n` cookie → `window.g_lang` →
 * navigator language → the product fallback. The resolution is written to the
 * canonical `v2board_locale` key; the legacy keys are read-only fallbacks here
 * and are never written again. This is the single permitted `g_lang` read.
 */
export function prepareI18nLocale(
  fallback: SupportedLocale,
  readCookie: (name: string) => string,
): SupportedLocale {
  if (typeof window === 'undefined') return fallback;

  const locale =
    resolveSupportedLocale(window.localStorage.getItem('v2board_locale')) ??
    resolveSupportedLocale(window.localStorage.getItem('umi_locale')) ??
    resolveSupportedLocale(readCookie('i18n')) ??
    resolveSupportedLocale(window.g_lang) ??
    resolveNavigatorLocale(window.navigator) ??
    fallback;
  // Canonicalize storage: leaving an unsupported old value in place would make
  // i18next fall back while API headers keep the stale value, so a successful
  // bootstrap always repairs the canonical key.
  window.localStorage.setItem('v2board_locale', locale);
  return locale;
}
