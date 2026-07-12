import type { CreateI18nOptions } from './index';
import { readCookie, setLocale } from './index';
import { prepareI18nLocale } from './bootstrap';
import { initializeI18n } from './instance';
import enUS from './locales/en-US';
import jaJP from './locales/ja-JP';
import koKR from './locales/ko-KR';
import viVN from './locales/vi-VN';
import zhCN from './locales/zh-CN';
import zhTW from './locales/zh-TW';

/** Test-only constructor that keeps all locale assertions synchronous. */
export function createI18n(options: CreateI18nOptions = {}) {
  const fallback = options.fallback ?? 'zh-CN';
  const lng = prepareI18nLocale(fallback, readCookie, setLocale);
  return initializeI18n({
    defaultNS: options.defaultNS,
    fallback,
    lazy: false,
    lng,
    resources: {
      'zh-CN': zhCN,
      'zh-TW': zhTW,
      'en-US': enUS,
      'ja-JP': jaJP,
      'vi-VN': viVN,
      'ko-KR': koKR,
      ...options.resources,
    },
  }).instance;
}

export { zhCN };
