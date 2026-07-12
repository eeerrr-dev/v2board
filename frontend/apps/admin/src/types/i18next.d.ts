import 'i18next';
import type { Translations } from '@v2board/i18n';

// Selector mode validates the real locale tree without recursively flattening
// hundreds of dotted string keys. `"optimize"` keeps editor/typecheck cost
// effectively constant for this large dictionary while preserving interpolation
// inference at each selector call. Dynamic backend messages go through the
// narrow translateRuntimeMessage bridge instead of weakening the app-wide type.
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    enableSelector: 'optimize';
    resources: {
      translation: Translations;
    };
  }
}
