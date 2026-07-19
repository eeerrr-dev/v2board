import dayjs from 'dayjs';
import type { SupportedLocale } from './locale-registry';

// dayjs locale packs self-register on import; activation is a separate,
// explicit dayjs.locale() call. 'en' is dayjs's built-in default and has no
// pack to load.
const DAYJS_LOCALES: Record<SupportedLocale, { code: string; load?: () => Promise<unknown> }> = {
  'zh-CN': { code: 'zh-cn', load: () => import('dayjs/locale/zh-cn') },
  'zh-TW': { code: 'zh-tw', load: () => import('dayjs/locale/zh-tw') },
  'en-US': { code: 'en' },
  'ja-JP': { code: 'ja', load: () => import('dayjs/locale/ja') },
  'vi-VN': { code: 'vi', load: () => import('dayjs/locale/vi') },
  'ko-KR': { code: 'ko', load: () => import('dayjs/locale/ko') },
};

let latestRequest = 0;

/**
 * Keeps dayjs's global locale in sync with the active app locale so any
 * locale-aware date rendering (month/weekday names, week start) matches the
 * UI language. Packs load on demand; when rapid switches race, the most
 * recent request wins regardless of load completion order.
 */
export async function applyDayjsLocale(locale: SupportedLocale): Promise<void> {
  const { code, load } = DAYJS_LOCALES[locale];
  const request = ++latestRequest;
  if (load) await load();
  if (request === latestRequest) dayjs.locale(code);
}
