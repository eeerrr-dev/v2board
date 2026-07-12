import i18n, { type i18n as I18nInstance } from 'i18next';
import resourcesToBackend from 'i18next-resources-to-backend';
import { initReactI18next } from 'react-i18next';
import type { SupportedLocale } from './locale-registry';
import { normalizeSupportedLocale } from './bootstrap';

type LocaleResources = Partial<Record<SupportedLocale, Record<string, unknown>>>;

const localeResourceLoaders: Record<
  SupportedLocale,
  () => Promise<{ default: Record<string, unknown> }>
> = {
  'zh-CN': () => import('./locales/zh-CN'),
  'zh-TW': () => import('./locales/zh-TW'),
  'en-US': () => import('./locales/en-US'),
  'ja-JP': () => import('./locales/ja-JP'),
  'vi-VN': () => import('./locales/vi-VN'),
  'ko-KR': () => import('./locales/ko-KR'),
};

let activeI18n: I18nInstance | undefined;

export function initializeI18n({
  defaultNS = 'translation',
  fallback,
  lazy,
  lng,
  resources = {},
}: {
  defaultNS?: string;
  fallback: SupportedLocale;
  lazy: boolean;
  lng: SupportedLocale;
  resources?: LocaleResources;
}): { instance: I18nInstance; initialized: Promise<unknown> } {
  const instance = i18n.createInstance();
  instance.use(initReactI18next);

  if (lazy) {
    instance.use(
      resourcesToBackend(async (language: string) => {
        const locale = normalizeSupportedLocale(language, fallback);
        return (await localeResourceLoaders[locale]()).default;
      }),
    );
  }

  const initialized = instance.init({
    lng,
    resources: Object.fromEntries(
      Object.entries(resources).map(([code, resource]) => [code, { translation: resource }]),
    ),
    partialBundledLanguages: lazy,
    fallbackLng: fallback,
    ns: ['translation'],
    defaultNS,
    interpolation: { escapeValue: false },
  });
  activeI18n = instance;
  return { instance, initialized };
}

export function getActiveI18n(): I18nInstance | undefined {
  return activeI18n;
}
