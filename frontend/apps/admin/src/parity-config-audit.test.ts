import { describe, expect, it } from 'vitest';
import {
  assertDialectRouteMap,
  assertRouteCoverage,
  assertSameOrderedList,
  assertSpecGroupCoverage,
  assertSubset,
  extractObjectArray,
  extractRouteArray,
  formatAuditSuccess,
  normalizeScenarioRoute,
  routePatternMatches,
} from '../../../scripts/parity-config-audit.mjs';
import { routeMap } from '../../../tests/lib/dialect/route-map.mjs';
import { interactions } from '../../../tests/lib/interaction-scenarios.mjs';
import { GROUP_NAMES } from '../../../tests/lib/spec-groups.mjs';

describe('parity config audit helpers', () => {
  it('normalizes canonical scenario paths back to application routes', () => {
    expect(normalizeScenarioRoute('/register?code=INVITE2026')).toBe('/register');
    expect(normalizeScenarioRoute('/')).toBe('/');
    expect(() => normalizeScenarioRoute('/#/register')).toThrow(
      /canonical route path, not a hash URL/,
    );
  });

  it('matches concrete paths against dynamic route patterns', () => {
    expect(routePatternMatches('/order/:trade_no', '/order/VISUAL2026110001')).toBe(true);
    expect(routePatternMatches('/ticket/:ticket_id', '/ticket/7')).toBe(true);
    expect(routePatternMatches('/ticket/:ticket_id', '/ticket')).toBe(false);
  });

  it('extracts route arrays and public route lists from source text', () => {
    const appSource = `
      export const USER_ROUTE_PATHS = [
        '/dashboard',
        '/order/:trade_no',
      ] as const;
      const USER_ROUTE_GUARD_OPTIONS = {
        authenticatedFallback: '/dashboard',
        guestFallback: '/login',
        publicRoutes: ['/', '/login'],
        routes: USER_ROUTE_PATHS,
      } as const;
    `;
    expect(extractRouteArray(appSource, 'USER_ROUTE_PATHS')).toEqual([
      '/dashboard',
      '/order/:trade_no',
    ]);
    expect(extractObjectArray(appSource, 'USER_ROUTE_GUARD_OPTIONS', 'publicRoutes')).toEqual([
      '/',
      '/login',
    ]);
  });

  it('fails when the Makefile scope list does not mirror the interaction modules', () => {
    expect(
      assertSameOrderedList(
        'INTERACTION_PARITY_SCENARIOS',
        ['user-login'],
        ['user-login', 'admin-dashboard'],
      ),
    ).toEqual([
      'INTERACTION_PARITY_SCENARIOS is missing labels from the interaction modules: admin-dashboard',
    ]);
  });

  it('fails when a viewport list references labels the parity viewports do not define', () => {
    expect(assertSubset('viewport list', ['desktop', 'wide'], ['desktop'])).toEqual([
      'viewport list has values outside the allowed set: wide',
    ]);
  });

  it('fails when routes are missing parity scenarios', () => {
    expect(
      assertRouteCoverage(
        'user coverage',
        ['/dashboard', '/profile'],
        [{ label: 'user-dashboard', route: '/dashboard' }],
      ),
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
      assertRouteCoverage(
        'admin coverage',
        ['/dashboard'],
        [
          { label: 'admin-dashboard', route: '/dashboard' },
          { label: 'admin-legacy', route: '/legacy' },
        ],
      ),
    ).toEqual([
      'admin coverage has parity scenarios for routes not declared by App.tsx: admin-legacy -> /legacy',
    ]);
  });

  it('audits the dialect route map as complete two-world rows with the §6.5/§6.9 ticket rows', () => {
    expect(assertDialectRouteMap(routeMap)).toEqual([]);
    expect(assertDialectRouteMap([{ id: 'x', legacy: { method: 'GET', path: '/x' } }])).toContain(
      'dialect route map: x is missing its modern shape',
    );
    expect(
      assertDialectRouteMap([
        {
          id: 'admin.tickets.list',
          legacy: { method: 'FETCH', path: 'ticket/fetch' },
          modern: { method: 'GET', path: '/{secure_path}/tickets' },
        },
      ]),
    ).toEqual([
      'dialect route map: admin.tickets.list legacy method FETCH is invalid',
      'dialect route map: admin.tickets.list legacy path must start with /',
      'dialect route map: required §6.5/§6.9 ticket row admin.tickets.get is missing',
      'dialect route map: required §6.5/§6.9 ticket row admin.tickets.replies.create is missing',
      'dialect route map: required §6.5/§6.9 ticket row admin.tickets.close is missing',
      'dialect route map: required §6.5/§6.9 ticket row staff.tickets.list is missing',
      'dialect route map: required §6.5/§6.9 ticket row staff.tickets.get is missing',
      'dialect route map: required §6.5/§6.9 ticket row staff.tickets.replies.create is missing',
      'dialect route map: required §6.5/§6.9 ticket row staff.tickets.close is missing',
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
        dialectRouteCount: 155,
        failures: [],
        interactionScenarioCount: 159,
        scenarioCount: 240,
        specGroupCount: 17,
        uiAppSpecificCount: 6,
        uiSharedPrimitiveCount: 31,
        uiSharedStylesheetCount: 2,
        userRouteCount: 16,
        viewportCount: 2,
      }),
    ).toBe(
      'Parity config audit OK: 240 parity scenarios and 159 interactions across 17 spec groups and 2 viewports, Makefile INTERACTION_PARITY_SCENARIOS mirrors the interaction modules, and App.tsx route definitions cover 16 user routes plus 19 admin routes. The dialect route map carries 155 well-formed two-world rows (incl. the §6.5 admin ticket rows). UI sync covers 31 shared primitives, 2 shared stylesheets, and 6 explicit app-only primitives.',
    );
  });
});
