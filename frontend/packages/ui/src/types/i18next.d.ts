import 'i18next';
import type { Translations } from '@v2board/i18n';

// The shared primitives use the same selector-typed locale tree as both apps.
// Dynamic resolver/backend copy crosses the narrow translateRuntimeMessage
// boundary instead of weakening selector types package-wide.
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    enableSelector: 'optimize';
    resources: {
      translation: Translations;
    };
  }
}
