import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { zhCN } from '@v2board/i18n/testing';

// Components rendered without an app-level provider still use the same real
// translation tree instead of making react-i18next warn and echo raw keys.
if (!i18n.isInitialized) {
  await i18n.use(initReactI18next).init({
    lng: 'zh-CN',
    fallbackLng: 'zh-CN',
    resources: { 'zh-CN': { translation: zhCN } },
    initAsync: false,
    interpolation: { escapeValue: false },
  });
}

// Vitest runs with globals:false, so Testing Library's self-registration
// (which probes for a global afterEach/beforeAll) never fires. Register the
// per-test DOM cleanup and the React act() environment explicitly. The legacy
// hand-rolled createRoot harnesses set IS_REACT_ACT_ENVIRONMENT themselves, so
// enabling it globally is compatible with both harnesses during the migration.
(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  cleanup();
});
