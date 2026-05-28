import i18next from 'i18next';

export function i18nGet(message: string): string {
  const namespaced = `errors.${message}`;
  if (i18next.exists(namespaced)) return i18next.t(namespaced);
  return message;
}
