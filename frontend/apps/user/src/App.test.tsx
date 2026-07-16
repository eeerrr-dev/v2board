import { isValidElement, type ReactElement } from 'react';
import { CancelledError, QueryClient } from '@tanstack/react-query';
import { screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMemoryRouter, type LoaderFunctionArgs, type RouteObject } from 'react-router';
import type * as ApiClientModule from '@v2board/api-client';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { apiClient } from '@/lib/api';
import { setAuthData } from '@/lib/auth';
import { userQueryOptions } from '@/lib/queries';
import { renderRoutes } from '@/test/render';
import {
  USER_APP_LAYOUT_ROUTE_PATHS,
  USER_HASH_ROUTE_OPTIONS,
  USER_ROUTE_PATHS,
  createLoginLoader,
  createRequireUserMiddleware,
  createUserRouter,
  createUserRoutes,
  rootUserRouteLoader,
} from './App';

const sessionApi = vi.hoisted(() => ({
  checkLogin: vi.fn(),
  commConfig: vi.fn(),
  fetchNotices: vi.fn(),
  getStat: vi.fn(),
  getSubscribe: vi.fn(),
  info: vi.fn(),
}));

vi.mock('@v2board/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof ApiClientModule>();
  return {
    ...actual,
    user: {
      ...actual.user,
      checkLogin: sessionApi.checkLogin,
      commConfig: sessionApi.commConfig,
      fetchNotices: sessionApi.fetchNotices,
      getStat: sessionApi.getStat,
      getSubscribe: sessionApi.getSubscribe,
      info: sessionApi.info,
    },
  };
});

beforeEach(() => {
  sessionApi.commConfig.mockResolvedValue({});
  sessionApi.fetchNotices.mockResolvedValue([]);
  sessionApi.getStat.mockResolvedValue({ pending_orders: 0, pending_tickets: 0 });
  sessionApi.getSubscribe.mockResolvedValue(null);
});

function loaderArgs(routePath: string, signal?: AbortSignal): LoaderFunctionArgs {
  return {
    request: new Request(`https://v2board.local${routePath}`, { signal }),
    params: {},
    context: {},
  } as unknown as LoaderFunctionArgs;
}

type SyncLoader = (args: LoaderFunctionArgs) => unknown;

type AuthMiddleware = ReturnType<typeof createRequireUserMiddleware>;

/** Invokes the auth middleware the way the router does on a navigation. The
 * gate relies on the router auto-advancing when next() is not called, so the
 * stub fails loudly if the middleware ever starts calling next() itself. */
function runAuthMiddleware(middleware: AuthMiddleware, routePath: string) {
  const next = () => {
    throw new Error('auth middleware must not call next()');
  };
  return middleware(
    loaderArgs(routePath) as unknown as Parameters<AuthMiddleware>[0],
    next as Parameters<AuthMiddleware>[1],
  );
}

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

async function catchAsyncRedirect(run: () => Promise<unknown>): Promise<Response> {
  let thrown: unknown;
  try {
    await run();
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
  const protectedBranch = children.find((route) => (route.middleware?.length ?? 0) > 0);
  const protectedChildren = protectedBranch?.children ?? [];
  return {
    root,
    children,
    guestLayout: children.find((route) => elementType(route) === GuestLayout),
    protectedBranch,
    appLayout: protectedChildren.find((route) => authGatedChildType(route) === AppLayout),
    ticketDetail: protectedChildren.find((route) => route.path === '/ticket/:ticket_id'),
    catchAll: children.find((route) => route.path === '*'),
  };
}

afterEach(() => {
  setAuthData(null);
  sessionApi.checkLogin.mockReset();
  sessionApi.commConfig.mockReset();
  sessionApi.fetchNotices.mockReset();
  sessionApi.getStat.mockReset();
  sessionApi.getSubscribe.mockReset();
  sessionApi.info.mockReset();
});

describe('user route table', () => {
  it('matches the bundled user route list exactly', () => {
    expect([...USER_ROUTE_PATHS]).toEqual([
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
    expect(USER_ROUTE_PATHS).not.toContain('/forget');
    expect(USER_ROUTE_PATHS).not.toContain('/plans');
    expect(USER_ROUTE_PATHS).not.toContain('/orders');
    expect(USER_ROUTE_PATHS).not.toContain('/tickets');
    expect(USER_ROUTE_PATHS).not.toContain('/nodes');
    expect(USER_ROUTE_PATHS).not.toContain('/home');
  });

  it('wires the hash-route normalization options for this app', () => {
    expect(USER_HASH_ROUTE_OPTIONS.authenticatedFallback).toBe('/dashboard');
    expect(USER_HASH_ROUTE_OPTIONS.guestFallback).toBe('/login');
    // Empty on purpose: an authenticated session may stay on /login etc.
    // instead of being force-bounced to the dashboard (route contract).
    expect(USER_HASH_ROUTE_OPTIONS.authenticatedPublicFallbackRoutes).toEqual([]);
    // Duplicated nested paths like /dashboard/plan recover against the full table.
    expect(USER_HASH_ROUTE_OPTIONS.nestedPrefixes).toEqual([...USER_ROUTE_PATHS]);
    expect(USER_HASH_ROUTE_OPTIONS.publicRoutes).toEqual([
      '/',
      '/login',
      '/register',
      '/forgetpassword',
    ]);
    expect(USER_HASH_ROUTE_OPTIONS.routes).toEqual([...USER_ROUTE_PATHS]);
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
    expect(typeof guestLayout!.children?.find((route) => route.path === '/login')?.loader).toBe(
      'function',
    );
    expect(
      guestLayout!.children
        ?.filter((route) => route.path !== '/login')
        .every((route) => route.loader === undefined),
    ).toBe(true);
  });

  it('mounts app pages under one shared RequireAuth-gated AppLayout behind the auth middleware', () => {
    const { children, protectedBranch, appLayout } = userRouteTree();

    expect(appLayout).toBeDefined();
    expect(
      protectedBranch!.children!.filter((route) => authGatedChildType(route) === AppLayout),
    ).toHaveLength(1);
    expect((appLayout!.element as ReactElement).key).toBeNull();
    // Layer 1 of the auth gate: one pathless middleware branch guards every
    // protected navigation (behavior covered below).
    expect(children.filter((route) => (route.middleware?.length ?? 0) > 0)).toHaveLength(1);
    expect(protectedBranch!.middleware).toHaveLength(1);
    expect(typeof protectedBranch!.middleware?.[0]).toBe('function');
    expect(appLayout!.loader).toBeUndefined();
    expect(appLayout!.children?.map((route) => route.path)).toEqual([
      ...USER_APP_LAYOUT_ROUTE_PATHS,
    ]);
  });

  it('keeps ticket details as a standalone auth-gated chat route outside the app shell', () => {
    const { appLayout, ticketDetail } = userRouteTree();

    expect(USER_APP_LAYOUT_ROUTE_PATHS).not.toContain('/ticket/:ticket_id');
    expect(appLayout!.children?.some((route) => route.path === '/ticket/:ticket_id')).toBe(false);
    expect(ticketDetail).toBeDefined();
    // Auth-gated like app pages (it sits under the same middleware branch —
    // that is how userRouteTree found it), but rendered through a bare outlet:
    // the original standalone chat window, not the shared shell.
    expect(authGatedChildType(ticketDetail)).toBe(RouteBoundaryOutlet);
    expect(ticketDetail!.loader).toBeUndefined();
    const detailIndex = ticketDetail!.children?.[0];
    expect(detailIndex?.index).toBe(true);
    expect(typeof detailIndex?.lazy).toBe('function');
  });

  it('gives every rendered page a lazy route module and keeps root redirect-only', () => {
    const { root, children, guestLayout, protectedBranch, appLayout, ticketDetail } =
      userRouteTree();
    const rootEntry = children.find((route) => route.path === '/');
    const pageRoutes = [
      ...(guestLayout!.children ?? []),
      ...(appLayout!.children ?? []),
      ...(ticketDetail!.children ?? []),
    ];

    expect(rootEntry).toBeDefined();
    expect(rootEntry!.lazy).toBeUndefined();
    expect(typeof rootEntry!.loader).toBe('function');
    expect(rootEntry!.errorElement).toBeTruthy();
    expect(pageRoutes).toHaveLength(USER_ROUTE_PATHS.length - 1);
    for (const route of pageRoutes) {
      expect(typeof route.lazy).toBe('function');
      expect(route.errorElement).toBeTruthy();
    }
    for (const route of [root, guestLayout!, protectedBranch!, appLayout!, ticketDetail!]) {
      expect(route.errorElement).toBeTruthy();
    }

  });
});

describe('hash-route normalization loaders (root + catch-all wiring)', () => {
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

  it('recovers duplicated nested paths onto their canonical route, keeping the query', () => {
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

  it('recovers a duplicated nested path from the catch-all before falling back', () => {
    setAuthData('token-xyz');
    expect(
      catchRedirect(() => unknown(loaderArgs('/dashboard/traffic'))).headers.get('Location'),
    ).toBe('/traffic');
  });
});

describe('native root route loader', () => {
  it('routes guests to login and authenticated sessions to the dashboard', () => {
    setAuthData(null);
    expect(catchRedirect(() => rootUserRouteLoader()).headers.get('Location')).toBe('/login');

    setAuthData('token-xyz');
    expect(catchRedirect(() => rootUserRouteLoader()).headers.get('Location')).toBe('/dashboard');
  });
});

describe('login existing-session loader', () => {
  it('does not probe without a stored auth token', async () => {
    setAuthData(null);
    const loader = createLoginLoader(
      new QueryClient({ defaultOptions: { queries: { retry: false } } }),
    );

    await expect(loader(loaderArgs('/login'))).resolves.toBeNull();
    expect(sessionApi.checkLogin).not.toHaveBeenCalled();
    expect(sessionApi.info).not.toHaveBeenCalled();
  });

  it('redirects a valid session and prewarms user info through the QueryClient', async () => {
    setAuthData('EXISTING_AUTH');
    sessionApi.checkLogin.mockResolvedValue({ is_login: true });
    sessionApi.info.mockResolvedValue({ email: 'user@example.com' });
    const queryClient = new QueryClient();
    const loader = createLoginLoader(queryClient);

    const response = await catchAsyncRedirect(() => loader(loaderArgs('/login?redirect=order')));

    expect(response.headers.get('Location')).toBe('/order');
    expect(sessionApi.checkLogin).toHaveBeenCalledTimes(1);
    expect(sessionApi.checkLogin).toHaveBeenCalledWith(apiClient, {
      signal: expect.any(AbortSignal),
    });
    await waitFor(() => expect(sessionApi.info).toHaveBeenCalledTimes(1));
    expect(queryClient.getQueryData(userQueryOptions.checkLogin().queryKey)).toEqual({
      is_login: true,
    });
  });

  it('clears an invalid stored session and leaves the login route active', async () => {
    setAuthData('STALE_AUTH');
    sessionApi.checkLogin.mockResolvedValue({ is_login: false });
    const loader = createLoginLoader(
      new QueryClient({ defaultOptions: { queries: { retry: false } } }),
    );

    await expect(loader(loaderArgs('/login'))).resolves.toBeNull();

    expect(localStorage.getItem('authorization')).toBeNull();
    expect(sessionApi.info).not.toHaveBeenCalled();
  });

  it('keeps the token and lets the route boundary own a session-probe transport failure', async () => {
    setAuthData('EXISTING_AUTH');
    const failure = new Error('network down');
    sessionApi.checkLogin.mockRejectedValue(failure);
    const loader = createLoginLoader(
      new QueryClient({ defaultOptions: { queries: { retry: false } } }),
    );

    await expect(loader(loaderArgs('/login'))).rejects.toBe(failure);

    expect(localStorage.getItem('authorization')).toBe('EXISTING_AUTH');
    expect(sessionApi.info).not.toHaveBeenCalled();
  });

  it('quietly settles an aborted session probe because the navigation has no UI owner', async () => {
    setAuthData('EXISTING_AUTH');
    sessionApi.checkLogin.mockRejectedValue(new DOMException('aborted', 'AbortError'));
    const loader = createLoginLoader(
      new QueryClient({ defaultOptions: { queries: { retry: false } } }),
    );

    await expect(loader(loaderArgs('/login'))).resolves.toBeNull();
    expect(localStorage.getItem('authorization')).toBe('EXISTING_AUTH');
  });

  it('quietly settles TanStack Query cancellation without masking real failures', async () => {
    setAuthData('EXISTING_AUTH');
    sessionApi.checkLogin.mockRejectedValue(new CancelledError());
    const loader = createLoginLoader(
      new QueryClient({ defaultOptions: { queries: { retry: false } } }),
    );

    await expect(loader(loaderArgs('/login'))).resolves.toBeNull();
    expect(localStorage.getItem('authorization')).toBe('EXISTING_AUTH');
  });

  it('aborts the TanStack request when the sole route-loader consumer is cancelled', async () => {
    setAuthData('EXISTING_AUTH');
    let querySignal: AbortSignal | undefined;
    sessionApi.checkLogin.mockImplementation(
      (_client: unknown, config: { signal: AbortSignal }) =>
        new Promise((_resolve, reject) => {
          querySignal = config.signal;
          config.signal.addEventListener('abort', () => reject(new Error('aborted')), {
            once: true,
          });
        }),
    );
    const loader = createLoginLoader(new QueryClient());
    const navigation = new AbortController();

    const pending = loader(loaderArgs('/login', navigation.signal));
    await waitFor(() => expect(sessionApi.checkLogin).toHaveBeenCalledTimes(1));
    navigation.abort();

    await expect(pending).resolves.toBeNull();
    expect(querySignal?.aborted).toBe(true);
    expect(localStorage.getItem('authorization')).toBe('EXISTING_AUTH');
  });

  it('never probes the stale session while a verify handoff is present', async () => {
    setAuthData('EXISTING_AUTH');
    const loader = createLoginLoader(new QueryClient());

    await expect(
      loader(loaderArgs('/login?verify=one-time-token&redirect=order')),
    ).resolves.toBeNull();

    expect(sessionApi.checkLogin).not.toHaveBeenCalled();
    expect(localStorage.getItem('authorization')).toBe('EXISTING_AUTH');
  });

  it('dedupes concurrent/StrictMode-equivalent loader probes and user-info prewarms', async () => {
    setAuthData('EXISTING_AUTH');
    let resolveSession: ((value: { is_login: boolean }) => void) | undefined;
    let querySignal: AbortSignal | undefined;
    sessionApi.checkLogin.mockImplementation(
      (_client: unknown, config: { signal: AbortSignal }) =>
        new Promise((resolve) => {
          querySignal = config.signal;
          resolveSession = resolve;
        }),
    );
    sessionApi.info.mockResolvedValue({ email: 'user@example.com' });
    const loader = createLoginLoader(new QueryClient());
    const firstNavigation = new AbortController();
    const secondNavigation = new AbortController();

    const first = loader(loaderArgs('/login', firstNavigation.signal));
    const second = loader(loaderArgs('/login', secondNavigation.signal));
    await waitFor(() => expect(sessionApi.checkLogin).toHaveBeenCalledTimes(1));
    firstNavigation.abort();
    expect(querySignal?.aborted).toBe(false);
    resolveSession?.({ is_login: true });

    const results = await Promise.allSettled([first, second]);
    expect(results[0]).toEqual({ status: 'fulfilled', value: null });
    expect(results[1]?.status).toBe('rejected');
    if (results[1]?.status === 'rejected') {
      expect(results[1].reason).toBeInstanceOf(Response);
      expect((results[1].reason as Response).headers.get('Location')).toBe('/dashboard');
    }
    expect(sessionApi.checkLogin).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(sessionApi.info).toHaveBeenCalledTimes(1));
  });

  it('normalizes browser-equivalent protocol-relative redirect bypasses', async () => {
    setAuthData('EXISTING_AUTH');
    sessionApi.checkLogin.mockResolvedValue({ is_login: true });
    sessionApi.info.mockResolvedValue({ email: 'user@example.com' });
    const loader = createLoginLoader(new QueryClient());
    const unsafe = encodeURIComponent('/\\evil.example/path');

    const response = await catchAsyncRedirect(() =>
      loader(loaderArgs(`/login?redirect=${unsafe}`)),
    );

    expect(response.headers.get('Location')).toBe('/dashboard');
  });
});

describe('user route auth navigation gate (layer 1: route middleware)', () => {
  it('redirects an unauthenticated navigation to /login with the return path encoded', async () => {
    setAuthData(null);
    const middleware = createRequireUserMiddleware(new QueryClient());

    let thrown: unknown;
    try {
      await runAuthMiddleware(middleware, '/dashboard?tab=orders');
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

  it('lets an authenticated navigation through and warms the user info query', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    // Pre-seed so ensureQueryData resolves from cache instead of hitting the network.
    queryClient.setQueryData(userQueryOptions.info().queryKey, {} as never);
    const ensureSpy = vi.spyOn(queryClient, 'ensureQueryData');
    const middleware = createRequireUserMiddleware(queryClient);

    const result = await runAuthMiddleware(middleware, '/dashboard');

    // Resolving without a value lets the router auto-advance to the loaders.
    expect(result).toBeUndefined();
    expect(ensureSpy).toHaveBeenCalledTimes(1);
    expect(ensureSpy.mock.calls[0]?.[0]?.queryKey).toEqual(userQueryOptions.info().queryKey);
    setAuthData(null);
  });

  it('lets the route boundary handle /user/info failures without fabricating a user', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, 'ensureQueryData').mockRejectedValue(new Error('auth required'));
    const middleware = createRequireUserMiddleware(queryClient);

    await expect(runAuthMiddleware(middleware, '/dashboard')).rejects.toThrow('auth required');
    expect(queryClient.getQueryData(userQueryOptions.info().queryKey)).toBeUndefined();
    setAuthData(null);
  });

  it('does not seed when the 403 teardown already cleared the session mid-flight', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, 'ensureQueryData').mockImplementation(async () => {
      // The API unauthorized handler clears the token before rejection propagates.
      setAuthData(null);
      throw new Error('auth required');
    });
    const middleware = createRequireUserMiddleware(queryClient);

    let thrown: unknown;
    try {
      await runAuthMiddleware(middleware, '/dashboard');
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBeInstanceOf(Response);
    expect((thrown as Response).headers.get('Location')).toBe('/login');
    expect(queryClient.getQueryData(userQueryOptions.info().queryKey)).toBeUndefined();
  });

  it('keeps previously fetched user info intact when a refresh fails', async () => {
    setAuthData('token-xyz');
    const queryClient = new QueryClient();
    queryClient.setQueryData(userQueryOptions.info().queryKey, { email: 'a@b.c' } as never);
    vi.spyOn(queryClient, 'ensureQueryData').mockRejectedValue(new Error('timeout'));
    const middleware = createRequireUserMiddleware(queryClient);

    await expect(runAuthMiddleware(middleware, '/dashboard')).rejects.toThrow('timeout');

    expect(queryClient.getQueryData(userQueryOptions.info().queryKey)).toEqual({
      email: 'a@b.c',
    });
    setAuthData(null);
  });
});

describe('user route dashboard prefetch loader', () => {
  it('wires a dashboard-only loader that warms subscribe/stat/notices/comm', () => {
    const queryClient = new QueryClient();
    const prefetchSpy = vi.spyOn(queryClient, 'prefetchQuery').mockResolvedValue(null as never);
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

    // No auth re-check inside the loader: the requireUser middleware has
    // already redirected unauthenticated navigations before loaders run.
    setAuthData('token-xyz');
    expect(prefetch(loaderArgs('/dashboard'))).toBeNull();
    const warmedKeys = prefetchSpy.mock.calls.map(([options]) => options.queryKey);
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
  it('reads the #/ hash entry location defined by the router contract', () => {
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
      expect(router.state.location.search).toBe(`?redirect=${encodeURIComponent('/ticket/123')}`);
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
    vi.spyOn(queryClient, 'prefetchQuery').mockResolvedValue(undefined);
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
