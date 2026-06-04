import i18n, { type i18n as I18nInstance } from 'i18next';
import { initReactI18next } from 'react-i18next';

import zhCN from './locales/zh-CN';
import type { Translations } from './locales/zh-CN';
import zhTW from './locales/zh-TW';
import enUS from './locales/en-US';
import jaJP from './locales/ja-JP';
import viVN from './locales/vi-VN';
import koKR from './locales/ko-KR';
import faIR from './locales/fa-IR';
import {
  createLegacySourceReverseMap,
  translateLegacyDictionary,
  type LegacyDictionary,
} from './locales/legacy-fallback';

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
  { code: 'zh-TW', label: '繁體中文' },
  { code: 'en-US', label: 'English' },
  { code: 'ja-JP', label: '日本語' },
  { code: 'vi-VN', label: 'Tiếng Việt' },
  { code: 'ko-KR', label: '한국어' },
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

type LegacyI18nMap = Record<string, LegacyDictionary>;

declare global {
  interface Window {
    g_lang?: string;
    g_langSeparator?: string;
  }
}

function getLegacyDictionary(locale: SupportedLocale): Record<string, string> | undefined {
  if (typeof window === 'undefined') return undefined;
  const i18nSettings = (window as unknown as { settings?: { i18n?: LegacyI18nMap } }).settings?.i18n;
  return i18nSettings?.[locale];
}

function isSupportedLocale(locale: string | null | undefined): locale is SupportedLocale {
  return SUPPORTED_LOCALES.some((item) => item.code === locale);
}

function isLegacyLocaleFormat(locale: string): boolean {
  const separator = window.g_langSeparator ?? '-';
  return new RegExp(`^([a-z]{2})${separator}?([A-Z]{2})?$`).test(locale);
}

function getCookie(name: string): string {
  if (typeof document === 'undefined') return '';
  return document.cookie.split('; ').reduce((value, item) => {
    const [key, raw] = item.split('=');
    return key === name && raw !== undefined ? decodeURIComponent(raw) : value;
  }, '');
}

function getLegacyBootstrapLocale(): string | undefined {
  const cookieLocale = getCookie('i18n');
  if (cookieLocale) return cookieLocale;
  return LEGACY_NAVIGATOR_LOCALES[window.navigator.language.split('-')[0] ?? ''];
}

function getLegacyProviderLocale(fallback: SupportedLocale): SupportedLocale {
  if (typeof window === 'undefined') return fallback;
  const stored = window.localStorage.getItem('umi_locale');
  if (isSupportedLocale(stored)) return stored;
  return fallback;
}

export function legacyGetLocale(): string {
  if (typeof window === 'undefined') return '';
  const separator = window.g_langSeparator ?? '-';
  const stored = window.localStorage.getItem('umi_locale') ?? '';
  const navigatorLocale =
    typeof window.navigator.language === 'string'
      ? window.navigator.language.split('-').join(separator)
      : '';
  return stored || window.g_lang || navigatorLocale;
}

export function legacySetLocale(locale: string | undefined, reload = true): void {
  if (typeof window === 'undefined') return;
  if (locale !== undefined && !isLegacyLocaleFormat(locale)) {
    throw new Error('setLocale lang format error');
  }
  if (legacyGetLocale() === locale) return;
  window.g_lang = locale;
  window.localStorage.setItem('umi_locale', locale || '');
  if (reload) window.location.reload();
  if (window.dispatchEvent) window.dispatchEvent(new Event('languagechange'));
}

function legacyLocale(locale: SupportedLocale, fallback: Translations): Translations {
  const dict = getLegacyDictionary(locale);
  const sourceReverse = createLegacySourceReverseMap(getLegacyDictionary('zh-CN'));
  return translateLegacyDictionary(dict ? zhCN : fallback, dict, sourceReverse);
}

export function createI18n(options: CreateI18nOptions = {}): I18nInstance {
  const instance = i18n.createInstance();
  const fallback = options.fallback ?? 'zh-CN';
  if (typeof window !== 'undefined') {
    const bootstrapLocale = getLegacyBootstrapLocale();
    if (bootstrapLocale) legacySetLocale(bootstrapLocale);
  }
  const lng = getLegacyProviderLocale(fallback);
  if (typeof window !== 'undefined') {
    window.g_lang = lng;
    window.g_langSeparator = '-';
  }
  instance
    .use(initReactI18next)
    .init({
      lng,
      resources: {
        'zh-CN': { translation: legacyLocale('zh-CN', zhCN) },
        'zh-TW': { translation: legacyLocale('zh-TW', zhTW) },
        'en-US': { translation: legacyLocale('en-US', enUS) },
        'ja-JP': { translation: legacyLocale('ja-JP', jaJP) },
        'vi-VN': { translation: legacyLocale('vi-VN', viVN) },
        'ko-KR': { translation: legacyLocale('ko-KR', koKR) },
        'fa-IR': { translation: legacyLocale('fa-IR', faIR) },
      },
      fallbackLng: fallback,
      ns: ['translation'],
      defaultNS: options.defaultNS ?? 'translation',
      interpolation: { escapeValue: false },
    });
  return instance;
}

export { zhCN, zhTW, enUS, jaJP, viVN, koKR, faIR };
