import { LOCALE_ENTRIES, isSupportedLocale, type SupportedLocale } from '@v2board/i18n';

// Derived from the single locale registry so a new locale needs no edit here.
const ERROR_DICTIONARIES: Record<SupportedLocale, Record<string, string>> = Object.fromEntries(
  LOCALE_ENTRIES.map((entry): [SupportedLocale, Record<string, string>] => [
    entry.code,
    entry.translations.errors,
  ]),
) as Record<SupportedLocale, Record<string, string>>;

export function i18nGet(message: string): string {
  const locale = getCurrentLocale();
  const legacy = window.settings?.i18n?.[locale]?.[message];
  if (legacy) return legacy;
  return ERROR_DICTIONARIES[locale]?.[message] ?? message;
}

// Resolves the locale for error-dictionary lookups. Intentionally NOT the i18n
// package's legacyGetLocale: this path must fall back to zh-CN (never
// navigator.language) before the provider stamps window.g_lang, so an error
// message is never resolved against an unselected browser locale. Keep the two
// readers separate — see errors.test.ts "falls back to zh-CN instead of
// navigator language".
export function getCurrentLocale(): SupportedLocale {
  return (
    toSupportedLocale(safeLocalStorageGet('umi_locale')) ??
    toSupportedLocale(window.g_lang) ??
    'zh-CN'
  );
}

function toSupportedLocale(locale: string | null | undefined): SupportedLocale | undefined {
  return isSupportedLocale(locale) ? locale : undefined;
}

// localStorage access can throw (private mode / storage disabled); fall back.
function safeLocalStorageGet(key: string): string {
  try {
    return window.localStorage.getItem(key) ?? '';
  } catch {
    return '';
  }
}
