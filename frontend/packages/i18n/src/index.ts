import i18n, { type i18n as I18nInstance } from 'i18next';
import { initReactI18next } from 'react-i18next';

import zhCN from './locales/zh-CN';
import type { Translations } from './locales/zh-CN';
import { legacyDictionaries } from './locales/legacy-dictionaries';
import {
  createLegacySourceReverseMap,
  translateLegacyDictionary,
  type LegacyDictionary,
} from './locales/legacy-fallback';
import {
  LOCALE_ENTRIES,
  LEGACY_NAVIGATOR_LOCALES,
  getLocaleDirection,
  isSupportedLocale,
  type SupportedLocale,
} from './locale-registry';

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
  return i18nSettings?.[locale] ?? legacyDictionaries[locale];
}

function normalizeSupportedLocale(
  locale: string | null | undefined,
  fallback: SupportedLocale,
): SupportedLocale {
  const safeFallback = isSupportedLocale(fallback) ? fallback : 'zh-CN';
  const raw = locale?.trim();
  if (!raw) return safeFallback;
  if (isSupportedLocale(raw)) return raw;

  const match = /^([a-z]{2})(?:[-_]?([a-z]{2}))?$/i.exec(raw);
  if (!match) return safeFallback;
  const language = match[1]!.toLowerCase();
  const region = match[2]?.toUpperCase();
  const normalized = region ? `${language}-${region}` : undefined;
  if (isSupportedLocale(normalized)) return normalized;
  return LEGACY_NAVIGATOR_LOCALES[language] ?? safeFallback;
}

function isLegacyLocaleFormat(locale: string): boolean {
  const separator = window.g_langSeparator ?? '-';
  return new RegExp(`^([a-z]{2})${separator}?([A-Z]{2})?$`).test(locale);
}

function normalizeLegacyBootstrapLocale(locale: string | null | undefined): string | undefined {
  const raw = locale?.trim();
  if (!raw) return undefined;
  const separator = window.g_langSeparator ?? '-';
  const match = /^([a-z]{2})(?:[-_]?([A-Z]{2}))?$/i.exec(raw);
  if (!match) return undefined;
  const language = match[1]!.toLowerCase();
  const region = match[2]?.toUpperCase();
  return region ? `${language}${separator}${region}` : language;
}

/**
 * Legacy cookie reader with the old frontend's parsing semantics (including
 * tolerating malformed URI encoding). Shared with the apps so the `i18n`
 * cookie contract is parsed by exactly one implementation.
 */
export function getLegacyCookie(name: string): string {
  if (typeof document === 'undefined') return '';
  return document.cookie.split('; ').reduce((value, item) => {
    const [key, raw] = item.split('=');
    if (key !== name || raw === undefined) return value;
    try {
      return decodeURIComponent(raw);
    } catch {
      return value;
    }
  }, '');
}

function getLegacyBootstrapLocale(): string | undefined {
  const cookieLocale = normalizeLegacyBootstrapLocale(getLegacyCookie('i18n'));
  if (cookieLocale) return cookieLocale;
  const navigatorLanguage = window.navigator.language?.trim().split(/[-_@]/)[0]?.toLowerCase();
  return LEGACY_NAVIGATOR_LOCALES[navigatorLanguage ?? ''];
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
  const persisted = window.localStorage.getItem('umi_locale') || window.g_lang;
  if (locale !== undefined && persisted === locale) return;
  window.g_lang = locale;
  window.localStorage.setItem('umi_locale', locale || '');
  if (reload) window.location.reload();
  if (window.dispatchEvent) window.dispatchEvent(new Event('languagechange'));
}

export function getLegacyLocaleClassName(
  locale: string | null | undefined,
  {
    fallback = 'zh-CN',
    includeLocale = true,
  }: { fallback?: SupportedLocale; includeLocale?: boolean } = {},
): string {
  if (!includeLocale) return '';
  return normalizeSupportedLocale(locale, fallback);
}

export function applyLocaleDocumentEnvironment(
  locale: string | null | undefined,
  fallback: SupportedLocale = 'zh-CN',
): SupportedLocale {
  const normalized = normalizeSupportedLocale(locale, fallback);
  const direction = getLocaleDirection(normalized);
  if (typeof document !== 'undefined') {
    document.documentElement.lang = normalized;
    document.documentElement.dir = direction;
    document.documentElement.dataset.locale = normalized;
    document.documentElement.dataset.textDirection = direction;
  }
  return normalized;
}

export function installLocaleDocumentEnvironment(
  instance: I18nInstance,
  fallback: SupportedLocale = 'zh-CN',
): () => void {
  const apply = (locale?: string) => {
    applyLocaleDocumentEnvironment(
      locale ?? instance.resolvedLanguage ?? instance.language,
      fallback,
    );
  };
  const onLanguageChanged = (locale: string) => apply(locale);

  apply();
  instance.on('languageChanged', onLanguageChanged);
  return () => instance.off('languageChanged', onLanguageChanged);
}

// Renders a locale's resource tree by dictionary-translating the zh-CN source
// tree. The full per-locale UI trees are no longer bundled (only their errors
// slice survives in the locale registry), so a locale without a dictionary
// (windowless environments only) serves the zh-CN copy as-is.
function legacyLocale(
  locale: SupportedLocale,
  sourceReverse: Map<string, string> | undefined,
): Translations {
  return translateLegacyDictionary(zhCN, getLegacyDictionary(locale), sourceReverse);
}

export function createI18n(options: CreateI18nOptions = {}): I18nInstance {
  const instance = i18n.createInstance();
  const fallback = options.fallback ?? 'zh-CN';
  if (typeof window !== 'undefined') {
    const bootstrapLocale = getLegacyBootstrapLocale();
    if (bootstrapLocale) legacySetLocale(bootstrapLocale, false);
  }
  const lng = getLegacyProviderLocale(fallback);
  if (typeof window !== 'undefined') {
    window.g_lang = lng;
    window.g_langSeparator = '-';
  }
  // The zh-CN reverse map is identical for every locale; build it once instead
  // of once per registered locale.
  const sourceReverse = createLegacySourceReverseMap(getLegacyDictionary('zh-CN'));
  instance
    .use(initReactI18next)
    .init({
      lng,
      resources: Object.fromEntries(
        LOCALE_ENTRIES.map((entry) => [
          entry.code,
          { translation: legacyLocale(entry.code, sourceReverse) },
        ]),
      ),
      fallbackLng: fallback,
      ns: ['translation'],
      defaultNS: options.defaultNS ?? 'translation',
      interpolation: { escapeValue: false },
    });
  return instance;
}

export { zhCN };
export type { Translations } from './locales/zh-CN';
export * from './locale-registry';
