import { isSupportedLocale, translateLoadedError, type SupportedLocale } from '@v2board/i18n';

export function i18nGet(message: string): string {
  const locale = getCurrentLocale();
  return translateLoadedError(locale, message);
}

// Resolves the locale for error-dictionary lookups. Intentionally separate from
// the package's getLocale: this path must fall back to zh-CN (never
// navigator.language) before the bootstrap migration writes v2board_locale, so
// an error message is never resolved against an unselected browser locale. Keep
// the two readers separate — see errors.test.ts "falls back to zh-CN instead of
// navigator language".
export function getCurrentLocale(): SupportedLocale {
  return toSupportedLocale(window.localStorage.getItem('v2board_locale')) ?? 'zh-CN';
}

function toSupportedLocale(locale: string | null | undefined): SupportedLocale | undefined {
  return isSupportedLocale(locale) ? locale : undefined;
}
