import { keyFromSelector, type SelectorParam } from 'i18next';

type TranslationLabels = Readonly<Record<string, string>>;
type TranslationValues = Record<string, unknown>;
type TranslationInput = SelectorParam | string;

/** Minimal selector-aware react-i18next test double for package-owned components. */
export function createTestTranslation(labels: TranslationLabels, language = 'zh-CN') {
  const t = (input: TranslationInput, values: TranslationValues = {}) => {
    const key = typeof input === 'function' ? keyFromSelector(input) : input;
    const fallback = typeof values.defaultValue === 'string' ? values.defaultValue : key;
    let label = labels[key] ?? fallback;

    Object.entries(values).forEach(([name, value]) => {
      if (name === 'defaultValue') return;
      label = label.replaceAll(`{{${name}}}`, String(value));
      label = label.replaceAll(`{${name}}`, String(value));
    });
    return label;
  };

  return {
    t,
    i18n: {
      language,
      exists: (key: string) => Object.hasOwn(labels, key),
      t: (key: string, values?: TranslationValues) => t(key, values),
    },
  };
}
