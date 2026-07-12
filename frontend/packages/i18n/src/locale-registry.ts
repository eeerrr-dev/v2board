export type TextDirection = 'ltr' | 'rtl';

interface LocaleDefinition {
  code: string;
  /** Native-language label shown in the language menu. */
  label: string;
  /** Document/text direction; drives `<html dir>` and `data-text-direction`. */
  dir: TextDirection;
  /** Primary navigator-language subtags that select this locale by default. */
  navigatorKeys: readonly string[];
}

/**
 * Single source of truth for locale identity, presentation metadata, browser
 * resolution, and the complete i18next resource registered for that locale.
 */
export const LOCALE_ENTRIES = [
  {
    code: 'zh-CN',
    label: '简体中文',
    dir: 'ltr',
    navigatorKeys: ['zh'],
  },
  {
    code: 'zh-TW',
    label: '繁體中文',
    dir: 'ltr',
    navigatorKeys: [],
  },
  {
    code: 'en-US',
    label: 'English',
    dir: 'ltr',
    navigatorKeys: ['en'],
  },
  {
    code: 'ja-JP',
    label: '日本語',
    dir: 'ltr',
    navigatorKeys: ['ja'],
  },
  {
    code: 'vi-VN',
    label: 'Tiếng Việt',
    dir: 'ltr',
    navigatorKeys: ['vi'],
  },
  {
    code: 'ko-KR',
    label: '한국어',
    dir: 'ltr',
    navigatorKeys: ['ko'],
  },
] as const satisfies readonly LocaleDefinition[];

export type LocaleEntry = (typeof LOCALE_ENTRIES)[number];
export type SupportedLocale = LocaleEntry['code'];

export const LOCALE_REGISTRY = Object.fromEntries(
  LOCALE_ENTRIES.map((entry) => [entry.code, entry]),
) as Record<SupportedLocale, LocaleEntry>;

export const SUPPORTED_LOCALES: { code: SupportedLocale; label: string }[] = LOCALE_ENTRIES.map(
  ({ code, label }) => ({ code, label }),
);

export const NAVIGATOR_LOCALES: Record<string, SupportedLocale> = Object.fromEntries(
  LOCALE_ENTRIES.flatMap((entry) => entry.navigatorKeys.map((key) => [key, entry.code])),
);

export function isSupportedLocale(locale: string | null | undefined): locale is SupportedLocale {
  return locale != null && Object.prototype.hasOwnProperty.call(LOCALE_REGISTRY, locale);
}

/** Text direction for a locale; unknown locales default to ltr. */
export function getLocaleDirection(locale: string | null | undefined): TextDirection {
  return isSupportedLocale(locale) ? LOCALE_REGISTRY[locale].dir : 'ltr';
}
