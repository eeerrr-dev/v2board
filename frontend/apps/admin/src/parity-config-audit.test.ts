import { describe, expect, it } from 'vitest';
import {
  assertRouteCoverage,
  assertSameOrderedValues,
  assertSubset,
  extractAssignedRouteArray,
  extractObjectArray,
  extractRouteArray,
  extractVisualScenarioPaths,
  formatAuditSuccess,
  normalizeScenarioRoute,
  routePatternMatches,
} from '../../../scripts/parity-config-audit.mjs';

describe('parity config audit helpers', () => {
  it('normalizes visual parity hash paths back to legacy routes', () => {
    expect(normalizeScenarioRoute('/#/register?code=INVITE2026')).toBe('/register');
    expect(normalizeScenarioRoute('/${adminPath}#/config/theme')).toBe('/config/theme');
    expect(normalizeScenarioRoute('/${adminPath}#/')).toBe('/');
  });

  it('matches concrete visual paths against dynamic legacy route patterns', () => {
    expect(routePatternMatches('/order/:trade_no', '/order/VISUAL2026110001')).toBe(true);
    expect(routePatternMatches('/ticket/:ticket_id', '/ticket/7')).toBe(true);
    expect(routePatternMatches('/ticket/:ticket_id', '/ticket')).toBe(false);
  });

  it('extracts route arrays and visual scenario paths from source text', () => {
    const appSource = `
      export const USER_LEGACY_ROUTE_PATHS = [
        '/dashboard',
        '/order/:trade_no',
      ] as const;
      const USER_LEGACY_ROUTE_OPTIONS = {
        authenticatedFallback: '/dashboard',
        guestFallback: '/login',
        publicRoutes: ['/', '/login'],
        routes: USER_LEGACY_ROUTE_PATHS,
      } as const;
    `;
    const devEntrySource = `
      <script>
        var legacyRoutes = [
          '/dashboard',
          '/order/:trade_no',
        ];
      </script>
    `;
    const scenarioBlock = `
      { label: 'user-dashboard', path: '/#/dashboard' },
      { label: 'user-order-detail', path: '/#/order/VISUAL2026110001' },
      { label: 'admin-theme', path: \`/\${adminPath}#/config/theme\` },
    `;

    expect(extractRouteArray(appSource, 'USER_LEGACY_ROUTE_PATHS')).toEqual([
      '/dashboard',
      '/order/:trade_no',
    ]);
    expect(extractObjectArray(appSource, 'USER_LEGACY_ROUTE_OPTIONS', 'publicRoutes')).toEqual([
      '/',
      '/login',
    ]);
    expect(extractAssignedRouteArray(devEntrySource, 'var legacyRoutes = [')).toEqual([
      '/dashboard',
      '/order/:trade_no',
    ]);
    expect(extractVisualScenarioPaths(scenarioBlock)).toEqual([
      { label: 'user-dashboard', route: '/dashboard' },
      { label: 'user-order-detail', route: '/order/VISUAL2026110001' },
      { label: 'admin-theme', route: '/config/theme' },
    ]);
  });

  it('fails when mirrored route lists drift', () => {
    expect(
      assertSameOrderedValues('dev entry routes', ['/dashboard'], ['/dashboard', '/plan']),
    ).toEqual(['dev entry routes is missing values: /plan']);
    expect(
      assertSameOrderedValues('dev entry routes', ['/plan', '/dashboard'], ['/dashboard', '/plan']),
    ).toEqual([
      'dev entry routes order differs.\nActual:   /plan /dashboard\nExpected: /dashboard /plan',
    ]);
  });

  it('fails when browser parity lists reference missing script labels', () => {
    expect(assertSubset('browser scenarios', ['user-login', 'missing'], ['user-login'])).toEqual([
      'browser scenarios has values not defined by visual-parity.mjs: missing',
    ]);
  });

  it('fails when routes are missing visual parity scenarios', () => {
    expect(
      assertRouteCoverage('user coverage', ['/dashboard', '/profile'], [
        { label: 'user-dashboard', route: '/dashboard' },
      ]),
    ).toEqual(['user coverage is missing screenshot scenarios for routes: /profile']);
  });

  it('fails when visual parity scenarios point at undeclared routes', () => {
    expect(
      assertRouteCoverage('admin coverage', ['/dashboard'], [
        { label: 'admin-dashboard', route: '/dashboard' },
        { label: 'admin-legacy', route: '/legacy' },
      ]),
    ).toEqual([
      'admin coverage has screenshot scenarios for routes not declared by App.tsx: admin-legacy -> /legacy',
    ]);
  });

  it('formats the success summary with route coverage counts', () => {
    expect(
      formatAuditSuccess({
        adminRouteCount: 19,
        browserScenarioCount: 12,
        browserViewportCount: 2,
        failures: [],
        interactionScenarioCount: 148,
        userRouteCount: 16,
        visualScenarioCount: 144,
      }),
    ).toBe(
      'Parity config audit OK: Makefile tracks 144 visual scenarios, 148 interaction scenarios, 12 browser scenarios across 2 viewports, parity covers 16 user routes plus 19 admin routes, and dev entry route mirrors are aligned.',
    );
  });
});
