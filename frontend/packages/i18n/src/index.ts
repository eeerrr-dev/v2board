import i18n, { type i18n as I18nInstance } from 'i18next';
import { initReactI18next } from 'react-i18next';

import zhCN from './locales/zh-CN';
import type { Translations } from './locales/zh-CN';
import zhTW from './locales/zh-TW';
import enUS from './locales/en-US';
import jaJP from './locales/ja-JP';
import faIR from './locales/fa-IR';

export type SupportedLocale =
  | 'zh-CN'
  | 'zh-TW'
  | 'en-US'
  | 'ja-JP'
  | 'vi-VN'
  | 'ko-KR'
  | 'fa-IR';

export const SUPPORTED_LOCALES: { code: SupportedLocale; label: string }[] = [
  { code: 'zh-CN', label: '简体中文' },
  { code: 'en-US', label: 'English' },
  { code: 'ja-JP', label: '日本語' },
  { code: 'vi-VN', label: 'Tiếng Việt' },
  { code: 'ko-KR', label: '한국어' },
  { code: 'zh-TW', label: '繁體中文' },
  { code: 'fa-IR', label: 'فارسی' },
];

export const RTL_LOCALES: SupportedLocale[] = ['fa-IR'];

const LEGACY_NAVIGATOR_LOCALES: Record<string, SupportedLocale> = {
  ja: 'ja-JP',
  zh: 'zh-CN',
  en: 'en-US',
  vi: 'vi-VN',
  ko: 'ko-KR',
};

export interface CreateI18nOptions {
  fallback?: SupportedLocale;
  defaultNS?: string;
}

type LegacyI18nMap = Record<string, Record<string, string>>;
type TranslationTree = string | number | boolean | null | TranslationTree[] | { [key: string]: TranslationTree };

function getLegacyDictionary(locale: SupportedLocale): Record<string, string> | undefined {
  if (typeof window === 'undefined') return undefined;
  const i18nSettings = (window as unknown as { settings?: { i18n?: LegacyI18nMap } }).settings?.i18n;
  return i18nSettings?.[locale];
}

function isSupportedLocale(locale: string | null | undefined): locale is SupportedLocale {
  return SUPPORTED_LOCALES.some((item) => item.code === locale);
}

function getCookie(name: string): string {
  if (typeof document === 'undefined') return '';
  return document.cookie.split('; ').reduce((value, item) => {
    const [key, raw] = item.split('=');
    return key === name && raw !== undefined ? decodeURIComponent(raw) : value;
  }, '');
}

function getLegacyInitialLocale(fallback: SupportedLocale): SupportedLocale {
  if (typeof window === 'undefined') return fallback;
  const stored =
    getCookie('i18n') || window.localStorage.getItem('umi_locale');
  if (isSupportedLocale(stored)) return stored;
  const language = window.navigator.language.split('-')[0];
  return (language ? LEGACY_NAVIGATOR_LOCALES[language] : undefined) ?? fallback;
}

function translateFromZh(tree: TranslationTree, dict: Record<string, string> | undefined): TranslationTree {
  if (typeof tree === 'string') return dict?.[tree] ?? tree;
  if (Array.isArray(tree)) return tree.map((item) => translateFromZh(item, dict));
  if (tree && typeof tree === 'object') {
    return Object.fromEntries(
      Object.entries(tree).map(([key, value]) => [key, translateFromZh(value, dict)]),
    );
  }
  return tree;
}

function legacyLocale(locale: SupportedLocale, fallback: Translations): Translations {
  const dict = getLegacyDictionary(locale);
  return dict ? (translateFromZh(zhCN, dict) as Translations) : fallback;
}

export function createI18n(options: CreateI18nOptions = {}): I18nInstance {
  const instance = i18n.createInstance();
  instance
    .use(initReactI18next)
    .init({
      lng: getLegacyInitialLocale(options.fallback ?? 'zh-CN'),
      resources: {
        'zh-CN': { translation: legacyLocale('zh-CN', zhCN) },
        'zh-TW': { translation: legacyLocale('zh-TW', zhTW) },
        'en-US': { translation: legacyLocale('en-US', enUS) },
        'ja-JP': { translation: legacyLocale('ja-JP', jaJP) },
        'vi-VN': { translation: legacyLocale('vi-VN', zhCN) },
        'ko-KR': { translation: legacyLocale('ko-KR', zhCN) },
        'fa-IR': { translation: legacyLocale('fa-IR', faIR) },
      },
      fallbackLng: options.fallback ?? 'zh-CN',
      ns: ['translation'],
      defaultNS: options.defaultNS ?? 'translation',
      interpolation: { escapeValue: false },
    });
  return instance;
}

export { zhCN, zhTW, enUS, jaJP, faIR };
