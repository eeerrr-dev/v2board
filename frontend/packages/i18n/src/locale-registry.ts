import zhCN, { type Translations } from './locales/zh-CN';
import zhTW from './locales/zh-TW';
import enUS from './locales/en-US';
import jaJP from './locales/ja-JP';
import viVN from './locales/vi-VN';
import koKR from './locales/ko-KR';

// Single source of truth for locale identity.
//
// The old umi frontend derived every locale-dependent fact from one map keyed by
// locale code plus the operator-controlled `window.settings.i18n` list. The rewrite
// had scattered the same fact across a `SupportedLocale` union, `SUPPORTED_LOCALES`,
// a navigator map, the i18next `resources` block, the error dictionaries, and a
// per-component antd-string map each — so adding or removing one locale meant editing
// ~8 files in lockstep (which is why fa-IR kept oscillating between commits).
//
// Everything locale-dependent now derives from `LOCALE_ENTRIES`. To add a locale,
// add one entry here; to remove one, delete one entry. No other file may enumerate
// the locale set.

export type SupportedLocale =
  | 'zh-CN'
  | 'zh-TW'
  | 'en-US'
  | 'ja-JP'
  | 'vi-VN'
  | 'ko-KR';

export type TextDirection = 'ltr' | 'rtl';

// The subset of an antd locale pack the user app must reproduce itself. The old
// frontend wrapped the app in antd's LocaleProvider, so antd supplied these strings
// (Empty description, Icon aria-label word, Modal OK/Cancel) from its per-locale pack.
// The user app intentionally has no antd runtime (it reimplements antd widgets as
// `legacy-*` components), so it reads the same strings from here instead. The admin
// app uses real antd v6 packs and does not consume this.
export interface AntdMessages {
  /** antd `<Empty>` default description. */
  emptyDescription: string;
  /** antd `<Icon>` aria-label word; antd v3 localizes this only for zh-CN. */
  iconWord: string;
  /** antd `Modal.confirm` default OK text. */
  okText: string;
  /** antd `Modal.confirm` default Cancel text. */
  cancelText: string;
}

export interface LocaleEntry {
  code: SupportedLocale;
  /** Native-language label shown in the language menu. */
  label: string;
  /** Document/text direction; drives `<html dir>` and `data-text-direction`. */
  dir: TextDirection;
  /**
   * `navigator.language` primary subtags that resolve to this locale when no stored
   * preference exists (e.g. `zh` -> zh-CN). zh-TW is intentionally empty: a bare `zh`
   * navigator maps to zh-CN, matching the old frontend.
   */
  navigatorKeys: string[];
  /** Bundled translation tree (raw-Chinese-keyed; see locales/legacy-fallback). */
  translations: Translations;
  /** Reproduced antd locale-pack strings for the no-antd user app. */
  antd: AntdMessages;
}

// Order is the language-menu display order and the i18next resource registration order.
export const LOCALE_ENTRIES: LocaleEntry[] = [
  {
    code: 'zh-CN',
    label: '简体中文',
    dir: 'ltr',
    navigatorKeys: ['zh'],
    translations: zhCN,
    antd: { emptyDescription: '暂无数据', iconWord: '图标', okText: '确 定', cancelText: '取 消' },
  },
  {
    code: 'zh-TW',
    label: '繁體中文',
    dir: 'ltr',
    navigatorKeys: [],
    translations: zhTW,
    antd: { emptyDescription: '無此資料', iconWord: 'icon', okText: '確 定', cancelText: '取 消' },
  },
  {
    code: 'en-US',
    label: 'English',
    dir: 'ltr',
    navigatorKeys: ['en'],
    translations: enUS,
    antd: { emptyDescription: 'No Data', iconWord: 'icon', okText: 'OK', cancelText: 'Cancel' },
  },
  {
    code: 'ja-JP',
    label: '日本語',
    dir: 'ltr',
    navigatorKeys: ['ja'],
    translations: jaJP,
    antd: { emptyDescription: 'データがありません', iconWord: 'icon', okText: 'OK', cancelText: 'キャンセル' },
  },
  {
    code: 'vi-VN',
    label: 'Tiếng Việt',
    dir: 'ltr',
    navigatorKeys: ['vi'],
    translations: viVN,
    antd: { emptyDescription: 'Trống', iconWord: 'icon', okText: 'Đồng ý', cancelText: 'Hủy' },
  },
  {
    code: 'ko-KR',
    label: '한국어',
    dir: 'ltr',
    navigatorKeys: ['ko'],
    translations: koKR,
    antd: { emptyDescription: '데이터 없음', iconWord: 'icon', okText: '확인', cancelText: '취소' },
  },
];

export const LOCALE_REGISTRY: Record<SupportedLocale, LocaleEntry> = Object.fromEntries(
  LOCALE_ENTRIES.map((entry) => [entry.code, entry]),
) as Record<SupportedLocale, LocaleEntry>;

export const SUPPORTED_LOCALES: { code: SupportedLocale; label: string }[] = LOCALE_ENTRIES.map(
  ({ code, label }) => ({ code, label }),
);

export const LEGACY_NAVIGATOR_LOCALES: Record<string, SupportedLocale> = Object.fromEntries(
  LOCALE_ENTRIES.flatMap((entry) => entry.navigatorKeys.map((key) => [key, entry.code])),
);

export function isSupportedLocale(locale: string | null | undefined): locale is SupportedLocale {
  return locale != null && Object.prototype.hasOwnProperty.call(LOCALE_REGISTRY, locale);
}

const FALLBACK_ANTD = LOCALE_REGISTRY['en-US'].antd;

/** Text direction for a locale; unknown locales default to ltr (matching the bundled set). */
export function getLocaleDirection(locale: string | null | undefined): TextDirection {
  return isSupportedLocale(locale) ? LOCALE_REGISTRY[locale].dir : 'ltr';
}

/** Reproduced antd locale-pack strings for a locale; unknown locales fall back to en-US. */
export function getLocaleAntdMessages(locale: string | null | undefined): AntdMessages {
  return isSupportedLocale(locale) ? LOCALE_REGISTRY[locale].antd : FALLBACK_ANTD;
}
