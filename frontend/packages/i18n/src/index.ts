import type { i18n as I18nInstance } from 'i18next';
import { getLocaleDirection, type SupportedLocale } from './locale-registry';
import {
  normalizeSupportedLocale,
  prepareI18nLocale,
  resolveNavigatorLocale,
  resolveSupportedLocale,
} from './bootstrap';
import { getActiveI18n, initializeI18n } from './instance';

export interface CreateI18nOptions {
  fallback?: SupportedLocale;
  defaultNS?: string;
  /** In-memory resources for deterministic tests or an explicitly bundled locale set. */
  resources?: Partial<Record<SupportedLocale, Record<string, unknown>>>;
}

declare global {
  interface Window {
    /**
     * Retired legacy global. Read exactly once by the §11 bootstrap migration
     * in `prepareI18nLocale`; nothing may assign or read it anywhere else
     * (docs/api-dialect.md §11).
     */
    g_lang?: string;
  }
}

/**
 * Reads a cookie while tolerating malformed URI encoding. This parser is shared
 * by both apps so all frontend-owned cookie contracts have one implementation.
 */
export function readCookie(name: string): string {
  if (typeof document === 'undefined') return '';
  return document.cookie.split('; ').reduce((value, item) => {
    // Split on the first '=' only: values written by other cookie owners may
    // legitimately contain '=' (e.g. base64 padding) and must not be truncated.
    const separator = item.indexOf('=');
    if (separator === -1) return value;
    const key = item.slice(0, separator);
    const raw = item.slice(separator + 1);
    if (key !== name) return value;
    try {
      return decodeURIComponent(raw);
    } catch {
      return value;
    }
  }, '');
}

export function writeCookie(
  name: string,
  value: string | number,
  minutes = 525600,
  path = '/',
  domain?: string,
): void {
  if (typeof document === 'undefined') return;
  const expires = new Date(Date.now() + minutes * 60_000).toUTCString();
  document.cookie =
    `${name}=${encodeURIComponent(value)};expires=${expires};path=${path}` +
    (domain ? `;domain=${domain}` : '');
}

/**
 * Reads the active locale from the canonical `v2board_locale` key
 * (docs/api-dialect.md §11). Legacy keys (`umi_locale`, the `i18n` cookie,
 * `window.g_lang`) are only consulted by the one-time bootstrap migration in
 * `prepareI18nLocale`, which writes the canonical key before app code runs.
 */
export function getLocale(): SupportedLocale | '' {
  if (typeof window === 'undefined') return '';
  return (
    resolveSupportedLocale(window.localStorage.getItem('v2board_locale')) ??
    resolveNavigatorLocale(window.navigator) ??
    'zh-CN'
  );
}

/** Persists the active locale to the canonical `v2board_locale` key (§11). */
export function setLocale(locale: string | undefined): void {
  if (typeof window === 'undefined') return;
  const normalized = locale === undefined ? undefined : resolveSupportedLocale(locale);
  if (locale !== undefined && normalized === undefined) {
    throw new Error('setLocale lang format error');
  }
  if (normalized === undefined) {
    window.localStorage.removeItem('v2board_locale');
    return;
  }
  window.localStorage.setItem('v2board_locale', normalized);
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

function configureI18n(
  options: CreateI18nOptions,
  lazy: boolean,
): {
  instance: I18nInstance;
  initialized: Promise<unknown>;
} {
  const fallback = options.fallback ?? 'zh-CN';
  const lng = prepareI18nLocale(fallback, readCookie);
  return initializeI18n({
    defaultNS: options.defaultNS,
    fallback,
    lazy,
    lng,
    resources: options.resources,
  });
}

/** Loads the selected locale and fallback through separate dynamic chunks. */
export async function createLazyI18n(options: CreateI18nOptions = {}): Promise<I18nInstance> {
  const { instance, initialized } = configureI18n(options, true);
  await initialized;
  return instance;
}

/** Resolves an already-loaded backend error dictionary without eagerly bundling all locales. */
export function translateLoadedError(locale: SupportedLocale, message: string): string {
  const value = getActiveI18n()?.getResource(locale, 'translation', `errors.${message}`);
  return typeof value === 'string' ? value : message;
}

export type { Translations } from './locales/zh-CN';
export * from './locale-registry';
