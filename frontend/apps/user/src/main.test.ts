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
    expect(mainSource).toContain('getNormalizedLegacyHashPath');
    expect(mainSource).toContain('const legacyHashRouteOptions = {');
    expect(mainSource).toContain("authenticatedFallback: '/dashboard'");
    expect(mainSource).toContain("canonicalPath: '/'");
    expect(mainSource).toContain("guestFallback: '/login'");
    expect(mainSource).toContain('nestedPrefixes: USER_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain("publicRoutes: ['/', '/login', '/register', '/forgetpassword']");
    expect(mainSource).toContain('routes: USER_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('normalizeLegacyHashRoute(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLegacyHashRouteNormalizer(legacyHashRouteOptions);');
    expect(mainSource).toContain('if (import.meta.env.DEV) {');
    expect(mainSource).toContain("const legacyRecoveryVersion = 'white-screen-recovery-15';");
    expect(mainSource).toContain(
      'storageKey: `v2board:white-screen-recovery:${legacyRecoveryVersion}`',
    );
    expect(mainSource).toContain(
      'storageKey: `v2board:dev-module-recovery:${legacyRecoveryVersion}`',
    );
    expect(mainSource).toContain('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);');
    expect(mainSource).toContain(
      'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 1000,\n  });',
    );
    expect(mainSource).toContain('installLegacyDevWhiteScreenFallback({ delay: 5000 });');
    expect(mainSource).toContain(
      '} else {\n  installLegacyWhiteScreenRecovery(legacyHashRouteOptions, legacyWhiteScreenRecoveryConfig);',
    );
    expect(mainSource.indexOf('if (import.meta.env.DEV) {')).toBeLessThan(
      mainSource.indexOf('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);'),
    );
    expect(mainSource.indexOf('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);')).toBeLessThan(
      mainSource.indexOf(
        'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 1000,\n  });',
      ),
    );
    expect(
      mainSource.indexOf(
        'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 1000,\n  });',
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

  it('keeps the app on HashRouter like the bundled theme', () => {
    expect(mainSource).toContain('HashRouter');
    expect(mainSource).toContain('useLocation');
    expect(mainSource).toContain('Navigate');
    expect(mainSource).toContain(
      "import { RouteBoundaryElement } from './components/route-error-boundary';",
    );
    expect(mainSource).toContain('<HashRouter>');
    expect(mainSource).toContain('<LegacyRouteGate>');
    expect(mainSource).toContain('</LegacyRouteGate>');
    expect(mainSource).toContain('<RouteBoundaryElement>');
    expect(mainSource).toContain('<App />');
  });

  it('keeps the browser-facing config barrel free of Vite-only helpers', () => {
    expect(configIndexSource).toContain("export * from './format';");
    expect(configIndexSource).toContain("export * from './legacy-hash-route';");
    expect(configIndexSource).not.toContain("export * from './vite';");
  });

  it('installs dev entry recovery before the Vite module graph loads', () => {
    expect(indexSource).toContain("var recoveryVersion = 'white-screen-recovery-15';");
    expect(indexSource).toContain("var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;");
    expect(indexSource).toContain('function clearOldRecoveryState()');
    expect(indexSource).toContain("'v2board:white-screen-recovery:',");
    expect(indexSource).toContain("'v2board:dev-module-recovery:',");
    expect(indexSource).toContain("key.indexOf(':' + recoveryVersion + ':') !== -1");
    expect(indexSource).toContain('clearOldRecoveryState();');
    expect(indexSource).toContain("text.indexOf('outdated optimize dep') !== -1");
    expect(indexSource).toContain("text.indexOf('/node_modules/.vite/') !== -1 &&");
    expect(indexSource).toContain("text.indexOf('module script') !== -1");
    expect(indexSource).not.toContain("text.indexOf('/node_modules/.vite/') !== -1\n          );");
    expect(indexSource).toContain('function legacyMainEmpty(root)');
    expect(indexSource).toContain("root.querySelector('#main-container .content')");
    expect(indexSource).toContain('return elementEmpty(root) || legacyMainEmpty(root);');
    expect(indexSource).toContain('if (appEmpty()) recover();');
    expect(indexSource).toContain("window.addEventListener('hashchange', schedule);");
    expect(indexSource).toContain("window.addEventListener('popstate', schedule);");
    expect(indexSource).toContain('new MutationObserver(schedule).observe(observerTarget');
    expect(indexSource).toContain("current.searchParams.set('__v2board_entry_recover'");
    expect(indexSource).toContain('data-v2board-white-screen-fallback="1"');
    expect(indexSource.indexOf("var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;")).toBeLessThan(
      indexSource.indexOf(
        '<script type="module" src="/src/main.tsx?v=20260605-white-screen-recovery-15"',
      ),
    );
  });
});
