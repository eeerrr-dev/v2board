import { describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { QueryClient } from '@tanstack/react-query';
import type { LoaderFunctionArgs } from 'react-router';
import {
  USER_APP_LAYOUT_ROUTE_PATHS,
  USER_LEGACY_ROUTE_PATHS,
  createDashboardPrefetchLoader,
  createRequireUserLoader,
  createUserRouter,
} from './App';
import { setAuthData } from '@/lib/auth';
import { userQueryOptions } from '@/lib/queries';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'App.tsx'), 'utf8');

function loaderArgs(routePath: string): LoaderFunctionArgs {
  return {
    request: new Request(`https://v2board.local${routePath}`),
    params: {},
    context: {},
  } as unknown as LoaderFunctionArgs;
}

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
    expect(source).toContain('authenticatedPublicFallbackRoutes: []');
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

  it('prefetches the dashboard queries from its route loader', () => {
    expect(source).toContain(
      'export function createDashboardPrefetchLoader(queryClient: QueryClient)',
    );
    expect(source).toContain(
      "path === '/dashboard' ? pageRoute(path, prefetchDashboard) : pageRoute(path)",
    );
  });

  it('builds the hash router with the dashboard static loader alongside its lazy component', () => {
    const router = createUserRouter(new QueryClient());
    expect(router.routes.length).toBeGreaterThan(0);
    router.dispose();
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

describe('user route auth entry gate (layer 1: route loader)', () => {
  it('redirects an unauthenticated entry to /login with the return path encoded', async () => {
    setAuthData(null);
    const loader = createRequireUserLoader(new QueryClient());

    let thrown: unknown;
    try {
      await loader(loaderArgs('/dashboard?tab=orders'));
    } catch (error) {
      thrown = error;
    }

    expect(thrown).toBeInstanceOf(Response);
    const response = thrown as Response;
    expect(response.status).toBe(302);
    expect(response.headers.get('Location')).toBe(
      `/login?redirect=${encodeURIComponent('/dashboard?tab=orders')}`,
    );
  });

  it('lets an authenticated entry through without redirecting', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    // Pre-seed so ensureQueryData resolves from cache instead of hitting the network.
    queryClient.setQueryData(userQueryOptions.info().queryKey, {} as never);
    const loader = createRequireUserLoader(queryClient);

    const result = await loader(loaderArgs('/dashboard'));

    expect(result).toBeNull();
    setAuthData(null);
  });
});

describe('user route dashboard prefetch loader', () => {
  it('warms subscribe/stat/notices/comm for an authenticated entry and skips them otherwise', () => {
    const queryClient = new QueryClient();
    // Pre-seed so ensureQueryData resolves from cache instead of hitting the network.
    queryClient.setQueryData(userQueryOptions.subscribe().queryKey, {} as never);
    queryClient.setQueryData(userQueryOptions.stat().queryKey, {} as never);
    queryClient.setQueryData(userQueryOptions.notices().queryKey, {} as never);
    queryClient.setQueryData(userQueryOptions.commConfig().queryKey, {} as never);
    const ensureSpy = vi.spyOn(queryClient, 'ensureQueryData');
    const prefetch = createDashboardPrefetchLoader(queryClient);

    setAuthData(null);
    expect(prefetch()).toBeNull();
    expect(ensureSpy).not.toHaveBeenCalled();

    setAuthData('token-xyz');
    expect(prefetch()).toBeNull();
    const warmedKeys = ensureSpy.mock.calls.map(([options]) => options.queryKey);
    expect(warmedKeys).toEqual([
      userQueryOptions.subscribe().queryKey,
      userQueryOptions.stat().queryKey,
      userQueryOptions.notices().queryKey,
      userQueryOptions.commConfig().queryKey,
    ]);
    setAuthData(null);
  });
});
