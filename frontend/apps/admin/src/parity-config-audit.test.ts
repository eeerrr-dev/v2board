import { describe, expect, it } from 'vitest';
import {
  assertRouteCoverage,
  assertSameOrderedList,
  assertSameOrderedValues,
  assertSpecGroupCoverage,
  assertSubset,
  extractAssignedRouteArray,
  extractObjectArray,
  extractRouteArray,
  formatAuditSuccess,
  normalizeScenarioRoute,
  routePatternMatches,
} from '../../../scripts/parity-config-audit.mjs';
import { interactions } from '../../../tests/lib/interaction-scenarios.mjs';
import { GROUP_NAMES } from '../../../tests/lib/spec-groups.mjs';

describe('parity config audit helpers', () => {
  it('normalizes parity hash paths back to legacy routes', () => {
    expect(normalizeScenarioRoute('/#/register?code=INVITE2026')).toBe('/register');
    expect(normalizeScenarioRoute('/${adminPath}#/config/theme')).toBe('/config/theme');
    expect(normalizeScenarioRoute('/${adminPath}#/')).toBe('/');
  });

  it('matches concrete paths against dynamic legacy route patterns', () => {
    expect(routePatternMatches('/order/:trade_no', '/order/VISUAL2026110001')).toBe(true);
    expect(routePatternMatches('/ticket/:ticket_id', '/ticket/7')).toBe(true);
    expect(routePatternMatches('/ticket/:ticket_id', '/ticket')).toBe(false);
  });

  it('extracts route arrays and public route lists from source text', () => {
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

  it('fails when the Makefile scope list does not mirror the interaction modules', () => {
    expect(
      assertSameOrderedList('INTERACTION_PARITY_SCENARIOS', ['user-login'], [
        'user-login',
        'admin-dashboard',
      ]),
    ).toEqual([
      'INTERACTION_PARITY_SCENARIOS is missing labels from the interaction modules: admin-dashboard',
    ]);
  });

  it('fails when a viewport list references labels the parity viewports do not define', () => {
    expect(assertSubset('viewport list', ['desktop', 'wide'], ['desktop'])).toEqual([
      'viewport list has values not defined by the parity viewports: wide',
    ]);
  });

  it('fails when routes are missing parity scenarios', () => {
    expect(
      assertRouteCoverage('user coverage', ['/dashboard', '/profile'], [
        { label: 'user-dashboard', route: '/dashboard' },
      ]),
    ).toEqual(['user coverage is missing parity scenarios for routes: /profile']);
  });

  it('retires pixel coverage only when an interaction scenario still gates the route', () => {
    const scenarios = [{ label: 'user-login', route: '/login', visualRetired: true }];

    expect(assertRouteCoverage('user coverage', ['/login'], scenarios, new Set())).toEqual([
      'user coverage retired pixel parity without interaction/behavior coverage for routes: /login (user-login)',
    ]);
    expect(
      assertRouteCoverage('user coverage', ['/login'], scenarios, new Set(['user-login'])),
    ).toEqual([]);
  });

  it('fails when parity scenarios point at undeclared routes', () => {
    expect(
      assertRouteCoverage('admin coverage', ['/dashboard'], [
        { label: 'admin-dashboard', route: '/dashboard' },
        { label: 'admin-legacy', route: '/legacy' },
      ]),
    ).toEqual([
      'admin coverage has parity scenarios for routes not declared by App.tsx: admin-legacy -> /legacy',
    ]);
  });

  it('maps every interaction to exactly one spec group with no empty groups', () => {
    expect(assertSpecGroupCoverage(interactions, GROUP_NAMES)).toEqual([]);
    expect(assertSpecGroupCoverage(interactions, [...GROUP_NAMES, 'phantom-group'])).toEqual([
      'Spec groups with no interactions: phantom-group',
    ]);
  });

  it('formats the success summary with scenario, interaction, and route counts', () => {
    expect(
      formatAuditSuccess({
        adminRouteCount: 19,
        interactionScenarioCount: 159,
        scenarioCount: 240,
        specGroupCount: 17,
        userRouteCount: 16,
        viewportCount: 2,
      }),
    ).toBe(
      'Parity config audit OK: 240 parity scenarios and 159 interactions across 17 spec groups and 2 viewports, Makefile INTERACTION_PARITY_SCENARIOS mirrors the interaction modules, parity covers 16 user routes plus 19 admin routes, and dev entry route mirrors are aligned.',
    );
  });
});
