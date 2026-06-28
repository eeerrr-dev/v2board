import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const indexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../index.html'),
  'utf8',
);
const configIndexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../packages/config/src/index.ts'),
  'utf8',
);

describe('user legacy entrypoint', () => {
  it('normalizes broken hash routes before rendering the user router', () => {
    expect(mainSource).toContain('installLegacyHashRouteNormalizer');
    expect(mainSource).toContain('installLegacyWhiteScreenRecovery');
    expect(mainSource).toContain('installLegacyDevModuleRecovery');
    expect(mainSource).toContain('installLegacyDevWhiteScreenFallback');
    expect(mainSource).toContain('normalizeLegacyHashRoute');
    expect(mainSource).toContain('installLocaleDocumentEnvironment');
    expect(mainSource).toContain('const legacyHashRouteOptions = {');
    expect(mainSource).toContain("authenticatedFallback: '/dashboard'");
    expect(mainSource).toContain('authenticatedPublicFallbackRoutes: []');
    expect(mainSource).toContain("canonicalPath: '/'");
    expect(mainSource).toContain("guestFallback: '/login'");
    expect(mainSource).toContain('nestedPrefixes: USER_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain("publicRoutes: ['/', '/login', '/register', '/forgetpassword']");
    expect(mainSource).toContain('routes: USER_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('normalizeLegacyHashRoute(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLegacyHashRouteNormalizer(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLocaleDocumentEnvironment(i18n);');
    expect(mainSource).toContain('if (import.meta.env.DEV) {');
    expect(mainSource).toContain("const legacyRecoveryVersion = 'white-screen-recovery-38';");
    expect(mainSource).toContain(
      'storageKey: `v2board:white-screen-recovery:${legacyRecoveryVersion}`',
    );
    expect(mainSource).toContain(
      'storageKey: `v2board:dev-module-recovery:${legacyRecoveryVersion}`',
    );
    expect(mainSource).toContain('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);');
    expect(mainSource).toContain(
      'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 3000,\n  });',
    );
    expect(mainSource).toContain('installLegacyDevWhiteScreenFallback({ delay: 5000 });');
    expect(mainSource).toContain(
      '} else {\n  installLegacyWhiteScreenRecovery(legacyHashRouteOptions, legacyWhiteScreenRecoveryConfig);',
    );
    expect(mainSource.indexOf('if (import.meta.env.DEV) {')).toBeLessThan(
      mainSource.indexOf('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);'),
    );
    expect(
      mainSource.indexOf('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);'),
    ).toBeLessThan(
      mainSource.indexOf(
        'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 3000,\n  });',
      ),
    );
    expect(
      mainSource.indexOf(
        'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 3000,\n  });',
      ),
    ).toBeLessThan(mainSource.indexOf('installLegacyDevWhiteScreenFallback({ delay: 5000 });'));
    expect(mainSource).toContain('const router = createUserRouter(queryClient);');
    expect(mainSource).not.toContain('function LegacyRouteGate');
    expect(mainSource).not.toContain('getNormalizedLegacyHashPath');
  });

  it('renders through the React Router data router', () => {
    expect(mainSource).toContain("import { RouterProvider } from 'react-router';");
    expect(mainSource).toContain("import { createUserRouter, USER_LEGACY_ROUTE_PATHS } from './App';");
    expect(mainSource).toContain('<RouterProvider router={router} />');
    expect(mainSource).toContain('<ConfirmDialogProvider />');
    expect(mainSource).toContain('<Toaster />');
    expect(mainSource).not.toContain('HashRouter');
    expect(mainSource).not.toContain('useLocation');
    expect(mainSource).not.toContain('Navigate');
  });

  it('keeps the browser-facing config barrel free of Vite-only helpers', () => {
    expect(configIndexSource).toContain("export * from './format';");
    expect(configIndexSource).toContain("export * from './legacy-hash-route';");
    expect(configIndexSource).not.toContain("export * from './vite';");
  });

  it('installs dev entry recovery before the Vite module graph loads', () => {
    expect(indexSource).toContain("var recoveryVersion = 'white-screen-recovery-38';");
    expect(indexSource).toContain(
      "var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;",
    );
    expect(indexSource).toContain('function clearOldRecoveryState()');
    expect(indexSource).toContain("'v2board:white-screen-recovery:',");
    expect(indexSource).toContain("'v2board:dev-module-recovery:',");
    expect(indexSource).toContain("key.indexOf(':' + recoveryVersion + ':') !== -1");
    expect(indexSource).toContain('clearOldRecoveryState();');
    expect(indexSource).toContain('function clearBrowserCaches()');
    expect(indexSource).toContain("if (!('caches' in window)) return;");
    expect(indexSource).toContain('clearBrowserCaches();');
    expect(indexSource).toContain('var legacyRoutes = [');
    expect(indexSource).toContain(
      "var legacyPublicRoutes = ['/', '/login', '/register', '/forgetpassword'];",
    );
    expect(indexSource).toContain(
      'var legacyAuthenticatedPublicFallbackRoutes = [];',
    );
    expect(indexSource).toContain('function normalizeBootUrl(url)');
    expect(indexSource).toContain("var nextHash = '#' + normalizedLegacyPath(routeSource);");
    expect(indexSource).toContain(
      "window.history.replaceState(window.history.state, '', bootUrl.toString());",
    );
    expect(indexSource).toContain('normalizeBootUrl(current);');
    expect(indexSource).toContain("text.indexOf('outdated optimize dep') !== -1");
    expect(indexSource).toContain("text.indexOf('/node_modules/.vite/') !== -1 &&");
    expect(indexSource).toContain("text.indexOf('module script') !== -1");
    expect(indexSource).not.toContain("text.indexOf('/node_modules/.vite/') !== -1\n          );");
    expect(indexSource).toContain('function routeMismatchWarning(value)');
    expect(indexSource).toContain("text.indexOf('no routes matched location') !== -1");
    expect(indexSource).toContain("text.indexOf('matched location \"/login/') !== -1");
    expect(indexSource).toContain('function patchConsoleRecovery(method)');
    expect(indexSource).toContain("patchConsoleRecovery('error');");
    expect(indexSource).toContain("patchConsoleRecovery('warn');");
    expect(indexSource).not.toContain('function legacyMainEmpty(root)');
    expect(indexSource).toContain('return elementEmpty(root);');
    expect(indexSource).not.toContain('legacyMainEmpty(root)');
    expect(indexSource).toContain("if (document.readyState === 'loading') {");
    expect(indexSource).toContain('if (appEmpty()) recover();');
    expect(indexSource).toContain("window.addEventListener('hashchange', schedule);");
    expect(indexSource).toContain("window.addEventListener('popstate', schedule);");
    expect(indexSource).toContain('new MutationObserver(schedule).observe(observerTarget');
    expect(indexSource).toContain("current.searchParams.set('__v2board_entry_recover'");
    expect(indexSource).toContain('data-v2board-white-screen-fallback="1"');
    expect(indexSource).not.toContain('/theme/default/assets/components.chunk.css');
    expect(indexSource).not.toContain('/theme/default/assets/umi.css');
    expect(indexSource).not.toContain('/theme/default/assets/vendors.async.js');
    expect(indexSource).not.toContain('/theme/default/assets/components.async.js');
    expect(indexSource).not.toContain('/theme/default/assets/i18n/');
    expect(indexSource).not.toContain('/theme/default/assets/');
    expect(
      indexSource.indexOf("var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;"),
    ).toBeLessThan(
      indexSource.indexOf(
        '<script type="module" src="/src/main.tsx?v=20260607-white-screen-recovery-38"',
      ),
    );
  });

  it('applies the dark mode cookie before first paint to avoid a theme flash', () => {
    expect(indexSource).toContain(
      "if (parts[0] !== 'dark_mode' || parts[1] === undefined) return value;",
    );
    expect(indexSource).toContain("document.documentElement.classList.add('dark');");
    expect(indexSource).toContain("document.documentElement.style.colorScheme = 'dark';");
    expect(indexSource.indexOf("if (mode === '1') {")).toBeLessThan(
      indexSource.indexOf('<script type="module" src="/src/main.tsx?'),
    );
  });
});
