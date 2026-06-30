import 'i18next';
import type { Translations } from '@v2board/i18n';

// Widen every leaf of the locale tree to `string` while preserving its nested
// key structure. This keeps compile-time checking of the *key names* (the value:
// it catches `t('profle.title')` typos across all ~700 keys) but deliberately
// drops i18next's per-key `{{placeholder}}` inference, whose recursive parse over
// a tree this large overflows the TS instantiation-depth limit (TS2589) on
// interpolating calls. Interpolation options stay loosely typed, which matches
// the legacy single->double-brace bridge anyway.
type WidenLeaves<T> = {
  [K in keyof T]: T[K] extends string ? string : WidenLeaves<T[K]>;
};

// Compile-time key checking for every t('...') call in the user app. The locale
// trees are code-structured (`common.submit`, `dashboard.title`), so the key
// shape of `typeof zhCN` is exactly what t() addresses with the default '.'
// keySeparator. This is a pure type augmentation: it never touches the runtime
// resources, the placeholder bridge, or language persistence. The few legacy
// flat-dictionary keys resolved at runtime (e.g. `t('Ticket does not exist')`)
// are cast at their call sites. Scoped to the user app's tsconfig (admin and the
// i18n package keep their existing loose typing).
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: {
      translation: WidenLeaves<Translations>;
    };
  }
}
