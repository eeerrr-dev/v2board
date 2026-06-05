import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const configIndexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../packages/config/src/index.ts'),
  'utf8',
);

describe('user legacy entrypoint', () => {
  it('normalizes broken hash routes before rendering the user router', () => {
    expect(mainSource).toContain('installLegacyHashRouteNormalizer');
    expect(mainSource).toContain('installLegacyWhiteScreenRecovery');
    expect(mainSource).toContain('installLegacyDevModuleRecovery');
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
    expect(mainSource).toContain('installLegacyWhiteScreenRecovery(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLegacyDevModuleRecovery();');
    expect(
      mainSource.indexOf('installLegacyWhiteScreenRecovery(legacyHashRouteOptions);'),
    ).toBeLessThan(mainSource.indexOf('if (import.meta.env.DEV) {'));
    expect(mainSource.indexOf('if (import.meta.env.DEV) {')).toBeLessThan(
      mainSource.indexOf('installLegacyDevModuleRecovery();'),
    );
    expect(mainSource).toContain("import type { ReactNode } from 'react';");
    expect(mainSource).toContain('function LegacyRouteGate({ children }: { children: ReactNode })');
    expect(mainSource).toContain('const normalized = getNormalizedLegacyHashPath(current, legacyHashRouteOptions);');
    expect(mainSource).toContain('return normalized !== current ? <Navigate to={normalized} replace /> : <>{children}</>;');
  });

  it('keeps the app on HashRouter like the bundled theme', () => {
    expect(mainSource).toContain('HashRouter');
    expect(mainSource).toContain('useLocation');
    expect(mainSource).toContain('Navigate');
    expect(mainSource).toContain("import { RouteBoundaryElement } from './components/route-error-boundary';");
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
});
