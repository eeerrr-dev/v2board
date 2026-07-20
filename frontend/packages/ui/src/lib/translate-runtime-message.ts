import type { i18n as I18nInstance } from 'i18next';

/**
 * Translate a key supplied at runtime when it exists in the shared resource
 * tree, otherwise preserve the backend- or resolver-provided copy.
 */
export function translateRuntimeMessage(i18n: I18nInstance, message: string): string {
  if (!i18n.exists(message)) return message;
  return i18n.t(message, { defaultValue: message });
}
