import { keyFromSelector, type SelectorParam } from 'i18next';

type TranslationLabels = Readonly<Record<string, string>>;
type TranslationValues = Record<string, unknown>;
type TranslationInput = SelectorParam | string;

export function testTranslationKey(input: TranslationInput): string {
  return typeof input === 'function' ? keyFromSelector(input) : input;
}

/** A selector-aware react-i18next test double with a minimal runtime-key API. */
export function createTestTranslation(labels: TranslationLabels, language = 'zh-CN') {
  const t = (input: TranslationInput, values: TranslationValues = {}) => {
    const key = testTranslationKey(input);
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

export type { TranslationInput, TranslationLabels, TranslationValues };
