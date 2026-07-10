import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const indexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../index.html'),
  'utf8',
);

describe('admin legacy entrypoint', () => {
  it('normalizes broken hash routes before rendering the admin router', () => {
    expect(mainSource).toContain('installLegacyHashRouteNormalizer');
    expect(mainSource).toContain('installLegacyWhiteScreenRecovery');
    expect(mainSource).toContain('installLegacyDevModuleRecovery');
    expect(mainSource).toContain('installLegacyDevWhiteScreenFallback');
    expect(mainSource).toContain('normalizeLegacyHashRoute');
    expect(mainSource).toContain('installLocaleDocumentEnvironment');
    expect(mainSource).toContain('getNormalizedLegacyHashPath');
    expect(mainSource).toContain('const legacyHashRouteOptions = {');
    expect(mainSource).toContain("authenticatedFallback: '/dashboard'");
    expect(mainSource).not.toContain("canonicalPath: '/'");
    expect(mainSource).toContain("guestFallback: '/login'");
    expect(mainSource).toContain("publicRoutes: ['/', '/login']");
    expect(mainSource).toContain('nestedPrefixes: ADMIN_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('routes: ADMIN_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('normalizeLegacyHashRoute(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLegacyHashRouteNormalizer(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLocaleDocumentEnvironment(i18n);');
    expect(mainSource).toContain('if (import.meta.env.DEV) {');
    expect(mainSource).toContain("const legacyRecoveryVersion = 'white-screen-recovery-37';");
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
    expect(mainSource).toContain("import { useEffect, type ReactNode } from 'react';");
    expect(mainSource).toContain('function LegacyRouteGate({ children }: { children: ReactNode })');
    expect(mainSource).toContain(
      'const normalized = getNormalizedLegacyHashPath(current, legacyHashRouteOptions);',
    );
    expect(mainSource).toContain('useEffect(() => {');
    expect(mainSource).toContain('normalizeLegacyHashRoute(legacyHashRouteOptions);');
    expect(mainSource).toContain('}, [location.hash, location.pathname, location.search]);');
    expect(mainSource).toContain(
      'return normalized !== current ? <Navigate to={normalized} replace /> : <>{children}</>;',
    );
  });

  it('initializes legacy settings and dark mode before rendering', () => {
    expect(mainSource).toContain('applyAdminLegacySettings();');
    expect(mainSource).toContain('applyInitialDarkMode();');
    const bootDarkModeIndex = mainSource.lastIndexOf('applyInitialDarkMode();');
    expect(mainSource.indexOf('applyAdminLegacySettings();')).toBeLessThan(
      bootDarkModeIndex,
    );
    expect(bootDarkModeIndex).toBeLessThan(mainSource.indexOf('const i18n = createI18n();'));
  });

  it('does not wrap the app in React StrictMode, matching the bundled admin entry', () => {
    expect(mainSource).not.toContain('StrictMode');
  });

  it('does not install timed query freshness or automatic retry absent from the bundled admin models', () => {
    expect(mainSource).toContain("import { redirectToLegacyLogin } from './lib/api';");
    expect(mainSource).toContain('queryCache: new QueryCache({');
    expect(mainSource).toContain('if (isUnauthorizedError(error)) redirectToLegacyLogin();');
    expect(mainSource).toContain('function isUnauthorizedError(error: unknown): boolean');
    expect(mainSource).toContain('const status = (error as { status?: unknown }).status;');
    expect(mainSource).toContain(
      "(error as { response?: { status?: unknown } }).response?.status",
    );
    expect(mainSource).toContain('return status === 403 || responseStatus === 403;');
    expect(mainSource).toContain(
      'defaultOptions: { queries: { staleTime: 0, retry: false, refetchOnWindowFocus: false } },',
    );
    expect(mainSource).not.toContain('staleTime: 30_000');
    expect(mainSource).not.toContain('retry: 1');
  });

  it('wraps the whole admin app with the white-screen guard inside HashRouter', () => {
    expect(mainSource).toContain('HashRouter');
    expect(mainSource).toContain('useLocation');
    expect(mainSource).toContain('Navigate');
    expect(mainSource).toContain(
      "import { RouteBoundaryElement } from './components/route-error-boundary';",
    );
    // The legacy antd confirm portal is gone; pages use the shadcn confirm
    // dialog + island toaster providers instead.
    expect(mainSource).not.toContain('LegacyConfirmProvider');
    expect(mainSource).toContain('<ConfirmDialogProvider />');
    expect(mainSource).toContain('<Toaster />');
    expect(mainSource).toContain('<HashRouter>');
    expect(mainSource).toContain('<LegacyRouteGate>');
    expect(mainSource).toContain('</LegacyRouteGate>');
    expect(mainSource).toContain('<RouteBoundaryElement>');
    expect(mainSource).toContain('<App />');
  });

  it('does not install a storage-event auth sync listener absent from the bundled admin entry', () => {
    expect(mainSource).not.toContain('setupAuthSync');
    expect(mainSource).not.toContain("from './lib/auth'");
  });

  it('no longer wraps the app in the antd ConfigProvider/App runtime', () => {
    // The admin surfaces are pure shadcn islands; the antd runtime provider and
    // its zh_CN locale were removed. API-error notifications now route through
    // the island Toaster instead of antd's static notification API.
    expect(mainSource).not.toContain("from 'antd'");
    expect(mainSource).not.toContain('antd/locale/zh_CN');
    expect(mainSource).not.toContain('ConfigProvider');
    expect(mainSource).not.toContain('AntdApp');
    expect(mainSource).toContain('<Toaster />');
  });

  it('installs dev entry recovery before the Vite module graph loads', () => {
    expect(indexSource).toContain("var recoveryVersion = 'white-screen-recovery-37';");
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
    expect(indexSource).toContain("var legacyPublicRoutes = ['/', '/login'];");
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
    expect(indexSource).not.toContain('/assets/admin/components.chunk.css');
    expect(indexSource).not.toContain('/assets/admin/umi.css');
    expect(indexSource).not.toContain('/assets/admin/vendors.async.js');
    expect(indexSource).not.toContain('/assets/admin/components.async.js');
    expect(
      indexSource.indexOf("var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;"),
    ).toBeLessThan(
      indexSource.indexOf(
        '<script type="module" src="/src/main.tsx?v=20260607-white-screen-recovery-37"',
      ),
    );
  });
});
