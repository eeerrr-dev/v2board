import { getLocale, setLocale, SUPPORTED_LOCALES, writeCookie } from '@v2board/i18n';
import { getRuntimeConfig } from './runtime-config';

const I18N_TEXT = Object.fromEntries(
  SUPPORTED_LOCALES.map((locale) => [locale.code, locale.label]),
);

export function getEnabledLocales() {
  // The enabled list comes from the backend's runtime config; drop any
  // locale the frontend no longer bundles a label/translation for instead of rendering
  // a blank menu item. Degrade to an empty menu if the backend omits the list rather
  // than throwing at render.
  return [...(getRuntimeConfig().i18n ?? [])]
    .sort()
    .filter((code) => code in I18N_TEXT)
    .map((code) => ({ code, label: I18N_TEXT[code] }));
}

export function getCurrentLocaleLabel() {
  return SUPPORTED_LOCALES.find((locale) => locale.code === getLocale())?.label;
}

// Persist the chosen locale across the public i18n cookie + umi_locale/g_lang contract.
// The caller drives i18next.changeLanguage so react-i18next re-renders in place and
// installLocaleDocumentEnvironment updates <html lang/dir> without a page reload.
export function selectLocale(locale: string) {
  writeCookie('i18n', locale);
  setLocale(locale);
}
