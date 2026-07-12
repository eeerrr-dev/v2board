import type { i18n as I18nInstance } from 'i18next';

/**
 * Translate a key supplied at runtime (validation or backend error text) when
 * it exists in our resources, otherwise preserve the server-provided copy.
 *
 * Application-owned keys use i18next's selector API. This is the deliberately
 * narrow boundary for strings that cannot be expressed as compile-time
 * selectors; `exists` refines the key before it reaches `t`.
 */
export function translateRuntimeMessage(i18n: I18nInstance, message: string): string {
  if (!i18n.exists(message)) return message;
  return i18n.t(message, { defaultValue: message });
}
