import { enUS, faIR, jaJP, zhCN, zhTW, type SupportedLocale } from '@v2board/i18n';
import { getLegacyCookie } from './legacy-cookie';

const ERROR_DICTIONARIES: Partial<Record<SupportedLocale, Record<string, string>>> = {
  'zh-CN': zhCN.errors,
  'zh-TW': zhTW.errors,
  'en-US': enUS.errors,
  'ja-JP': jaJP.errors,
  'fa-IR': faIR.errors,
};

export function i18nGet(message: string): string {
  const locale = getCurrentLocale();
  const legacy = window.settings?.i18n?.[locale]?.[message];
  if (legacy) return legacy;
  return ERROR_DICTIONARIES[locale]?.[message] ?? message;
}

function getCurrentLocale(): SupportedLocale {
  const locale =
    getLegacyCookie('i18n') ||
    window.localStorage.getItem('umi_locale') ||
    window.navigator.language;
  if (locale.startsWith('zh-TW')) return 'zh-TW';
  if (locale.startsWith('zh')) return 'zh-CN';
  if (locale.startsWith('ja')) return 'ja-JP';
  if (locale.startsWith('fa')) return 'fa-IR';
  if (locale.startsWith('en')) return 'en-US';
  if (locale.startsWith('vi')) return 'vi-VN';
  if (locale.startsWith('ko')) return 'ko-KR';
  return 'zh-CN';
}
