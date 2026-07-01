import { legacyGetLocale, legacySetLocale, SUPPORTED_LOCALES } from '@v2board/i18n';
import { setLegacyCookie } from '@/lib/legacy-cookie';

const I18N_TEXT = Object.fromEntries(SUPPORTED_LOCALES.map((locale) => [locale.code, locale.label]));

export function getEnabledLocales() {
  // The enabled list comes from the operator backend (window.settings.i18n); drop any
  // locale the frontend no longer bundles a label/translation for instead of rendering
  // a blank menu item. Degrade to an empty menu if the backend omits the list rather
  // than throwing at render.
  return [...(window.settings?.i18n ?? [])]
    .sort()
    .filter((code) => code in I18N_TEXT)
    .map((code) => ({ code, label: I18N_TEXT[code] }));
}

export function getCurrentLocaleLabel() {
  return SUPPORTED_LOCALES.find((locale) => locale.code === legacyGetLocale())?.label;
}

// Persist the chosen locale (i18n cookie + umi_locale/g_lang via legacySetLocale)
// WITHOUT the legacy full-page reload. The caller drives i18next.changeLanguage so
// react-i18next re-renders in place and installLocaleDocumentEnvironment updates
// <html lang/dir> off the languageChanged event.
export function selectLocale(locale: string) {
  setLegacyCookie('i18n', locale);
  legacySetLocale(locale, false);
}
