import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { USER_APP_LAYOUT_ROUTE_PATHS, USER_LEGACY_ROUTE_PATHS } from './App';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'App.tsx'), 'utf8');

describe('user legacy route table', () => {
  it('matches the bundled user route list exactly', () => {
    expect([...USER_LEGACY_ROUTE_PATHS]).toEqual([
      '/dashboard',
      '/forgetpassword',
      '/',
      '/invite',
      '/knowledge',
      '/login',
      '/node',
      '/order/:trade_no',
      '/order',
      '/plan/:plan_id',
      '/plan',
      '/profile',
      '/register',
      '/ticket/:ticket_id',
      '/ticket',
      '/traffic',
    ]);
  });

  it('does not expose route aliases absent from the bundled theme', () => {
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/forget');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/plans');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/orders');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/tickets');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/nodes');
    expect(USER_LEGACY_ROUTE_PATHS).not.toContain('/home');
  });

  it('keeps ticket details as the original standalone chat route', () => {
    expect(USER_APP_LAYOUT_ROUTE_PATHS).not.toContain('/ticket/:ticket_id');
    expect(source).toContain("path: '/ticket/:ticket_id'");
    expect(source).toContain("lazy: lazyPage('/ticket/:ticket_id')");
    expect(source).toContain('<RouteBoundaryOutlet />');
    expect(source).toContain('<RequireAuth>');
  });

  it('normalizes unmatched legacy hashes in route loaders before rendering pages', () => {
    expect(source).toContain("path: '*'");
    expect(source).toContain('export function normalizeUserRouteLoader');
    expect(source).toContain('export function unknownUserRouteLoader');
    expect(source).toContain('nestedPrefixes: USER_LEGACY_ROUTE_PATHS');
    expect(source).toContain('getNormalizedLegacyHashPath(current, USER_LEGACY_ROUTE_OPTIONS)');
    expect(source).toContain('if (normalized !== current) throw redirect(normalized);');
    expect(source).toContain("import { getAuthData } from '@/lib/auth';");
    expect(source).toContain('function matchesUserLegacyRoute(pathname: string): boolean');
    expect(source).toContain('matchPath({ path, end: true }, pathname)');
    expect(source).toContain('function getUserRouteFallback(): string');
    expect(source).toContain('throw redirect(getUserRouteFallback())');
    expect(source).not.toContain('<Routes>');
    expect(source).not.toContain('LegacyUnknownRouteRedirect');
  });

  it('uses React Router data APIs with lazy route modules and route error boundaries', () => {
    expect(source).toContain('createHashRouter');
    expect(source).toContain('export function createUserRoutes(queryClient: QueryClient)');
    expect(source).toContain('export function createRequireUserLoader(queryClient: QueryClient)');
    expect(source).toContain('await queryClient.ensureQueryData(userQueryOptions.info())');
    expect(source).toContain('function lazyPage(path: UserLegacyRoutePath)');
    expect(source).toContain('lazy: lazyPage(path)');
    expect(source).toContain('errorElement: <RouteErrorFallback />');
    expect(source).not.toContain('USER_ROUTE_ELEMENTS');
    expect(source).not.toContain('<Suspense');
  });

  it('keeps the shared layout mounted while switching routes', () => {
    expect(source).toContain('element: <GuestLayout />');
    expect(source).toContain('<RequireAuth>');
    expect(source).toContain('<AppLayout />');
    expect(source).not.toContain('key={routeComponentKey');
    expect(source).not.toContain('function KeyedAppLayout');
    expect(source).not.toContain('function KeyedGuestLayout');
  });
});
