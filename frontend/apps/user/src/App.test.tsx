import { isValidElement, type ReactElement } from 'react';
import { QueryClient } from '@tanstack/react-query';
import { screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createMemoryRouter, type LoaderFunctionArgs, type RouteObject } from 'react-router';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { setAuthData } from '@/lib/auth';
import { userQueryOptions } from '@/lib/queries';
import { renderRoutes } from '@/test/render';
import {
  USER_APP_LAYOUT_ROUTE_PATHS,
  USER_LEGACY_ROUTE_OPTIONS,
  USER_LEGACY_ROUTE_PATHS,
  createRequireUserLoader,
  createUserRouter,
  createUserRoutes,
} from './App';

function loaderArgs(routePath: string): LoaderFunctionArgs {
  return {
    request: new Request(`https://v2board.local${routePath}`),
    params: {},
    context: {},
  } as unknown as LoaderFunctionArgs;
}

type SyncLoader = (args: LoaderFunctionArgs) => unknown;

/** Runs a loader expected to throw a 302 redirect Response and returns it. */
function catchRedirect(run: () => unknown): Response {
  let thrown: unknown;
  try {
    run();
  } catch (error) {
    thrown = error;
  }
  expect(thrown).toBeInstanceOf(Response);
  const response = thrown as Response;
  expect(response.status).toBe(302);
  return response;
}

function elementType(route: RouteObject | undefined): unknown {
  return route && isValidElement(route.element) ? route.element.type : undefined;
}

/** The component a <RequireAuth> gate wraps, or undefined when not gated. */
function authGatedChildType(route: RouteObject | undefined): unknown {
  if (!route || !isValidElement(route.element) || route.element.type !== RequireAuth) {
    return undefined;
  }
  const child = (route.element.props as { children?: unknown }).children;
  return isValidElement(child) ? child.type : undefined;
}

function userRouteTree(queryClient = new QueryClient()) {
  const [root] = createUserRoutes(queryClient);
  if (!root) throw new Error('user route tree has no root route');
  const children = root.children ?? [];
  return {
    root,
    children,
    guestLayout: children.find((route) => elementType(route) === GuestLayout),
    appLayout: children.find((route) => authGatedChildType(route) === AppLayout),
    ticketDetail: children.find((route) => route.path === '/ticket/:ticket_id'),
    catchAll: children.find((route) => route.path === '*'),
  };
}

afterEach(() => {
  setAuthData(null);
});

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

  it('wires the legacy hash-normalization options for this app', () => {
    expect(USER_LEGACY_ROUTE_OPTIONS.authenticatedFallback).toBe('/dashboard');
    expect(USER_LEGACY_ROUTE_OPTIONS.guestFallback).toBe('/login');
    // Empty on purpose: an authenticated session may stay on /login etc.
    // instead of being force-bounced to the dashboard (legacy behavior).
    expect(USER_LEGACY_ROUTE_OPTIONS.authenticatedPublicFallbackRoutes).toEqual([]);
    // Nested legacy paths like /dashboard/plan recover against the full table.
    expect(USER_LEGACY_ROUTE_OPTIONS.nestedPrefixes).toEqual([...USER_LEGACY_ROUTE_PATHS]);
    expect(USER_LEGACY_ROUTE_OPTIONS.publicRoutes).toEqual([
      '/',
      '/login',
      '/register',
      '/forgetpassword',
    ]);
    expect(USER_LEGACY_ROUTE_OPTIONS.routes).toEqual([...USER_LEGACY_ROUTE_PATHS]);
  });
});

describe('user route tree', () => {
  it('mounts guest pages under one shared GuestLayout element', () => {
    const { children, guestLayout } = userRouteTree();

    expect(guestLayout).toBeDefined();
    // A single unkeyed layout element keeps the layout mounted across
    // sibling navigation instead of remounting per route.
    expect(children.filter((route) => elementType(route) === GuestLayout)).toHaveLength(1);
    expect((guestLayout!.element as ReactElement).key).toBeNull();
    expect(guestLayout!.children?.map((route) => route.path)).toEqual([
      '/login',
      '/register',
      '/forgetpassword',
    ]);
  });

  it('mounts app pages under one shared RequireAuth-gated AppLayout with the entry loader', () => {
    const { children, appLayout } = userRouteTree();

    expect(appLayout).toBeDefined();
    expect(children.filter((route) => authGatedChildType(route) === AppLayout)).toHaveLength(1);
    expect((appLayout!.element as ReactElement).key).toBeNull();
    // Layer 1 of the auth gate: the entry loader (behavior covered below).
    expect(typeof appLayout!.loader).toBe('function');
    expect(appLayout!.children?.map((route) => route.path)).toEqual([
      ...USER_APP_LAYOUT_ROUTE_PATHS,
    ]);
  });

  it('keeps ticket details as a standalone auth-gated chat route outside the app shell', () => {
    const { appLayout, ticketDetail } = userRouteTree();

    expect(USER_APP_LAYOUT_ROUTE_PATHS).not.toContain('/ticket/:ticket_id');
    expect(appLayout!.children?.some((route) => route.path === '/ticket/:ticket_id')).toBe(false);
    expect(ticketDetail).toBeDefined();
    // Auth-gated like app pages, but rendered through a bare outlet — the
    // original standalone chat window, not the shared shell.
    expect(authGatedChildType(ticketDetail)).toBe(RouteBoundaryOutlet);
    expect(typeof ticketDetail!.loader).toBe('function');
    const detailIndex = ticketDetail!.children?.[0];
    expect(detailIndex?.index).toBe(true);
    expect(typeof detailIndex?.lazy).toBe('function');
  });

  it('gives every legacy page a lazy route module with its own error boundary', async () => {
    const { root, children, guestLayout, appLayout, ticketDetail } = userRouteTree();
    const home = children.find((route) => route.path === '/');
    const pageRoutes = [
      home!,
      ...(guestLayout!.children ?? []),
      ...(appLayout!.children ?? []),
      ...(ticketDetail!.children ?? []),
    ];

    expect(pageRoutes).toHaveLength(USER_LEGACY_ROUTE_PATHS.length);
    for (const route of pageRoutes) {
      expect(typeof route.lazy).toBe('function');
      expect(route.errorElement).toBeTruthy();
    }
    for (const route of [root, guestLayout!, appLayout!, ticketDetail!]) {
      expect(route.errorElement).toBeTruthy();
    }

    // A lazy module resolves to a renderable route Component.
    const homeModule = await (home!.lazy as () => Promise<{ Component: unknown }>)();
    expect(typeof homeModule.Component).toBe('function');
  });
});

describe('legacy hash normalization loaders (root + catch-all wiring)', () => {
  const { root, catchAll } = userRouteTree();
  const normalize = root.loader as SyncLoader;
  const unknown = catchAll!.loader as SyncLoader;

  it('passes canonical paths through without redirecting', () => {
    setAuthData(null);
    expect(normalize(loaderArgs('/login'))).toBeNull();

    setAuthData('token-xyz');
    expect(normalize(loaderArgs('/dashboard?tab=orders'))).toBeNull();
  });

  it('lets an authenticated session stay on public auth routes (no forced dashboard bounce)', () => {
    setAuthData('token-xyz');
    expect(normalize(loaderArgs('/login'))).toBeNull();
    expect(normalize(loaderArgs('/register'))).toBeNull();
  });

  it('recovers nested legacy paths onto their canonical route, keeping the query', () => {
    setAuthData('token-xyz');
    const response = catchRedirect(() => normalize(loaderArgs('/dashboard/plan?from=email')));
    expect(response.headers.get('Location')).toBe('/plan?from=email');
  });

  it('normalizes the production #/ hash form of the request URL', () => {
    setAuthData('token-xyz');
    const response = catchRedirect(() =>
      normalize({
        request: new Request('https://v2board.local/#/dashboard/knowledge'),
        params: {},
        context: {},
      } as unknown as LoaderFunctionArgs),
    );
    expect(response.headers.get('Location')).toBe('/knowledge');
  });

  it('bounces a guest requesting a guarded path to /login', () => {
    setAuthData(null);
    const response = catchRedirect(() => normalize(loaderArgs('/knowledge')));
    expect(response.headers.get('Location')).toBe('/login');
  });

  it('sends unknown routes to the session fallback: guests to /login, authenticated to /dashboard', () => {
    setAuthData(null);
    expect(catchRedirect(() => unknown(loaderArgs('/bogus'))).headers.get('Location')).toBe(
      '/login',
    );

    setAuthData('token-xyz');
    expect(catchRedirect(() => unknown(loaderArgs('/bogus'))).headers.get('Location')).toBe(
      '/dashboard',
    );
  });

  it('recovers a nested legacy path from the catch-all before falling back', () => {
    setAuthData('token-xyz');
    expect(
      catchRedirect(() => unknown(loaderArgs('/dashboard/traffic'))).headers.get('Location'),
    ).toBe('/traffic');
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

  it('lets an authenticated entry through and warms the user info query', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    // Pre-seed so ensureQueryData resolves from cache instead of hitting the network.
    queryClient.setQueryData(userQueryOptions.info().queryKey, {} as never);
    const ensureSpy = vi.spyOn(queryClient, 'ensureQueryData');
    const loader = createRequireUserLoader(queryClient);

    const result = await loader(loaderArgs('/dashboard'));

    expect(result).toBeNull();
    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expect(ensureSpy.mock.calls[0]?.[0]?.queryKey).toEqual(userQueryOptions.info().queryKey);
    setAuthData(null);
  });

  it('seeds the legacy empty user record when /user/info fails while still authenticated', async () => {
    // Legacy keeps the dashboard shell mounted on an HTTP 401 from /user/info
    // (user-auth-401-no-redirect); an errored info query would make AppLayout's
    // useSuspenseQuery re-throw to the route errorElement and unmount the shell.
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, 'ensureQueryData').mockRejectedValue(new Error('auth required'));
    const loader = createRequireUserLoader(queryClient);

    const result = await loader(loaderArgs('/dashboard'));

    expect(result).toBeNull();
    expect(queryClient.getQueryData(userQueryOptions.info().queryKey)).toMatchObject({
      email: '',
      balance: 0,
    });
    setAuthData(null);
  });

  it('does not seed when the 403 teardown already cleared the session mid-flight', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, 'ensureQueryData').mockImplementation(async () => {
      // redirectToLegacyLogin clears the token before the rejection propagates.
      setAuthData(null);
      throw new Error('auth required');
    });
    const loader = createRequireUserLoader(queryClient);

    const result = await loader(loaderArgs('/dashboard'));

    expect(result).toBeNull();
    expect(queryClient.getQueryData(userQueryOptions.info().queryKey)).toBeUndefined();
  });

  it('keeps previously fetched user info instead of overwriting it with the fallback', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    queryClient.setQueryData(userQueryOptions.info().queryKey, { email: 'a@b.c' } as never);
    vi.spyOn(queryClient, 'ensureQueryData').mockRejectedValue(new Error('timeout'));
    const loader = createRequireUserLoader(queryClient);

    await loader(loaderArgs('/dashboard'));

    expect(queryClient.getQueryData(userQueryOptions.info().queryKey)).toEqual({
      email: 'a@b.c',
    });
    setAuthData(null);
  });
});

describe('user route dashboard prefetch loader', () => {
  it('wires a dashboard-only loader that warms subscribe/stat/notices/comm when authenticated', () => {
    const queryClient = new QueryClient();
    const ensureSpy = vi
      .spyOn(queryClient, 'ensureQueryData')
      .mockResolvedValue(null as never);
    const { appLayout } = userRouteTree(queryClient);
    const dashboard = appLayout!.children?.find((route) => route.path === '/dashboard');

    expect(typeof dashboard?.loader).toBe('function');
    // Only the dashboard entry warms queries; sibling pages have no loader.
    expect(
      appLayout!.children
        ?.filter((route) => route.path !== '/dashboard')
        .every((route) => route.loader === undefined),
    ).toBe(true);

    const prefetch = dashboard!.loader as SyncLoader;

    setAuthData(null);
    expect(prefetch(loaderArgs('/dashboard'))).toBeNull();
    expect(ensureSpy).not.toHaveBeenCalled();

    setAuthData('token-xyz');
    expect(prefetch(loaderArgs('/dashboard'))).toBeNull();
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

describe('user router behavior', () => {
  it('reads the legacy #/ hash entry location (hash router contract)', () => {
    setAuthData(null);
    window.location.hash = '#/login';
    const router = createUserRouter(new QueryClient());
    try {
      expect(router.routes.length).toBeGreaterThan(0);
      expect(router.state.location.pathname).toBe('/login');
    } finally {
      router.dispose();
      window.location.hash = '';
    }
  });

  it('redirects an unauthenticated ticket-detail entry to /login with the return path', async () => {
    setAuthData(null);
    const router = createMemoryRouter(createUserRoutes(new QueryClient()), {
      initialEntries: ['/ticket/123'],
    });
    try {
      await waitFor(() => expect(router.state.location.pathname).toBe('/login'), {
        timeout: 5000,
      });
      expect(router.state.location.search).toBe(
        `?redirect=${encodeURIComponent('/ticket/123')}`,
      );
    } finally {
      router.dispose();
    }
  });

  it('lands unknown hashes on the session fallback route', async () => {
    setAuthData(null);
    const guestRouter = createMemoryRouter(createUserRoutes(new QueryClient()), {
      initialEntries: ['/definitely/not/a/route'],
    });
    try {
      await waitFor(() => expect(guestRouter.state.location.pathname).toBe('/login'), {
        timeout: 5000,
      });
    } finally {
      guestRouter.dispose();
    }

    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, 'ensureQueryData').mockResolvedValue(null as never);
    const authedRouter = createMemoryRouter(createUserRoutes(queryClient), {
      initialEntries: ['/definitely/not/a/route'],
    });
    try {
      await waitFor(() => expect(authedRouter.state.location.pathname).toBe('/dashboard'), {
        timeout: 5000,
      });
    } finally {
      authedRouter.dispose();
    }
  });

  it('paints a role=status spinner while pending initial loaders hydrate the root', async () => {
    // Without a hydrate fallback the data router leaves #root empty while the
    // first navigation's loaders run — the DOM shape the retired production
    // white-screen watchdog keyed on.
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, 'ensureQueryData').mockReturnValue(new Promise<never>(() => {}));

    renderRoutes(createUserRoutes(queryClient), { i18n: true, initialEntries: ['/dashboard'] });

    expect(await screen.findByRole('status')).toBeInTheDocument();
  });
});
