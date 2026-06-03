import {
  SUPPORTED_LOCALES,
  enUS,
  faIR,
  jaJP,
  koKR,
  viVN,
  zhCN,
  zhTW,
  type SupportedLocale,
} from '@v2board/i18n';

const ERROR_DICTIONARIES: Partial<Record<SupportedLocale, Record<string, string>>> = {
  'zh-CN': zhCN.errors,
  'zh-TW': zhTW.errors,
  'en-US': enUS.errors,
  'ja-JP': jaJP.errors,
  'vi-VN': viVN.errors,
  'ko-KR': koKR.errors,
  'fa-IR': faIR.errors,
};

export function i18nGet(message: string): string {
  const locale = getCurrentLocale();
  const legacy = window.settings?.i18n?.[locale]?.[message];
  if (legacy) return legacy;
  return ERROR_DICTIONARIES[locale]?.[message] ?? message;
}

export function getCurrentLocale(): SupportedLocale {
  return (
    toSupportedLocale(safeLocalStorageGet('umi_locale')) ??
    toSupportedLocale(window.g_lang) ??
    'zh-CN'
  );
}

function toSupportedLocale(locale: string | null | undefined): SupportedLocale | undefined {
  return SUPPORTED_LOCALES.some((item) => item.code === locale)
    ? (locale as SupportedLocale)
    : undefined;
}

// localStorage access can throw (private mode / storage disabled); fall back.
function safeLocalStorageGet(key: string): string {
  try {
    return window.localStorage.getItem(key) ?? '';
  } catch {
    return '';
  }
}
