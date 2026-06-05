import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const antdCompatSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/antd-v5-compat.css'),
  'utf8',
);

describe('admin legacy entrypoint', () => {
  it('normalizes broken hash routes before rendering the admin router', () => {
    expect(mainSource).toContain('installLegacyHashRouteNormalizer');
    expect(mainSource).toContain('installLegacyWhiteScreenRecovery');
    expect(mainSource).toContain('installLegacyDevModuleRecovery');
    expect(mainSource).toContain('normalizeLegacyHashRoute');
    expect(mainSource).toContain('getNormalizedLegacyHashPath');
    expect(mainSource).toContain('const legacyHashRouteOptions = {');
    expect(mainSource).toContain("authenticatedFallback: '/dashboard'");
    expect(mainSource).toContain("canonicalPath: '/'");
    expect(mainSource).toContain("guestFallback: '/login'");
    expect(mainSource).toContain("publicRoutes: ['/', '/login']");
    expect(mainSource).toContain('nestedPrefixes: ADMIN_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('routes: ADMIN_LEGACY_ROUTE_PATHS');
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

  it('initializes legacy settings and dark mode before rendering', () => {
    expect(mainSource).toContain('applyAdminLegacySettings();\napplyInitialDarkMode();');
  });

  it('does not wrap the app in React StrictMode, matching the bundled admin entry', () => {
    expect(mainSource).not.toContain('StrictMode');
  });

  it('does not install timed query freshness or automatic retry absent from the bundled admin models', () => {
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
    expect(mainSource).toContain("import { RouteBoundaryElement } from './components/route-error-boundary';");
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

  it('keeps the admin Ant Design locale fixed to zh_CN like the bundled admin app', () => {
    expect(mainSource).toContain("import zhCN from 'antd/locale/zh_CN';");
    expect(mainSource).toContain('locale={zhCN}');
    expect(mainSource).not.toContain("antd/locale/en_US");
  });

  it('keeps Ant Design 5 table spin wrappers visible under the legacy admin stylesheet', () => {
    expect(mainSource).toContain("import './styles/antd-v5-compat.css';");
    expect(antdCompatSource).toContain('.ant-table-wrapper > .ant-spin');
    expect(antdCompatSource).toContain('display: block;');
    expect(antdCompatSource).not.toMatch(/^\.ant-spin\s*\{/m);
  });
});
