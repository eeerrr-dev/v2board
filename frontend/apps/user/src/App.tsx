import { CancelledError, type QueryClient } from '@tanstack/react-query';
import { type ComponentType } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createHashRouter,
  matchPath,
  redirect,
  type LoaderFunctionArgs,
  type RouteObject,
} from 'react-router';
import { getNormalizedHashPath } from '@v2board/config';
import type { CheckLoginResult } from '@v2board/types';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet, RouteErrorFallback } from '@/components/route-error-boundary';
import { Spinner } from '@/components/ui/spinner';
import {
  buildLoginRedirect,
  getAuthData,
  normalizeLoginRedirectTarget,
  setAuthData,
} from '@/lib/auth';
import { userKeys, userQueryOptions } from '@/lib/queries';

export const USER_ROUTE_PATHS = [
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
] as const;

type UserRoutePath = (typeof USER_ROUTE_PATHS)[number];
type UserPageRoutePath = Exclude<UserRoutePath, '/'>;

export const USER_GUEST_ROUTE_PATHS = ['/login', '/register', '/forgetpassword'] as const;

export const USER_APP_LAYOUT_ROUTE_PATHS = [
  '/dashboard',
  '/plan',
  '/plan/:plan_id',
  '/order',
  '/order/:trade_no',
  '/profile',
  '/invite',
  '/ticket',
  '/knowledge',
  '/node',
  '/traffic',
] as const;

export const USER_HASH_ROUTE_OPTIONS = {
  matchRoute: (route: string, path: string, end: boolean) => matchPath({ path: route, end }, path),
  authenticatedFallback: '/dashboard',
  authenticatedPublicFallbackRoutes: [],
  guestFallback: '/login',
  nestedPrefixes: USER_ROUTE_PATHS,
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_ROUTE_PATHS,
} as const;

type LazyRouteModule = Promise<{ default: ComponentType }>;

const USER_ROUTE_MODULES: Record<UserPageRoutePath, () => LazyRouteModule> = {
  '/dashboard': () => import('@/pages/dashboard'),
  '/forgetpassword': () => import('@/pages/auth/forget'),
  '/invite': () => import('@/pages/invite'),
  '/knowledge': () => import('@/pages/knowledge'),
  '/login': () => import('@/pages/auth/login'),
  '/node': () => import('@/pages/node'),
  '/order/:trade_no': () => import('@/pages/orders/detail'),
  '/order': () => import('@/pages/orders'),
  '/plan/:plan_id': () => import('@/pages/plans/checkout'),
  '/plan': () => import('@/pages/plans'),
  '/profile': () => import('@/pages/profile'),
  '/register': () => import('@/pages/auth/register'),
  '/ticket/:ticket_id': () => import('@/pages/tickets/detail'),
  '/ticket': () => import('@/pages/tickets'),
  '/traffic': () => import('@/pages/traffic'),
};

function lazyPage(path: UserPageRoutePath): RouteObject['lazy'] {
  return async () => {
    const module = await USER_ROUTE_MODULES[path]();
    return { Component: module.default };
  };
}

function getRequestRoutePath(request: Request): string {
  const url = new URL(request.url);
  if (url.hash.startsWith('#/')) return url.hash.slice(1);
  return `${url.pathname}${url.search}`;
}

function matchesUserRoute(pathname: string): boolean {
  return USER_ROUTE_PATHS.some((path) => matchPath({ path, end: true }, pathname));
}

function getUserRouteFallback(): string {
  return getAuthData()
    ? USER_HASH_ROUTE_OPTIONS.authenticatedFallback
    : USER_HASH_ROUTE_OPTIONS.guestFallback;
}

export function normalizeUserRouteLoader({ request }: LoaderFunctionArgs) {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedHashPath(current, USER_HASH_ROUTE_OPTIONS);

  if (normalized !== current) throw redirect(normalized);
  return null;
}

export function unknownUserRouteLoader({ request }: LoaderFunctionArgs) {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedHashPath(current, USER_HASH_ROUTE_OPTIONS);
  const url = new URL(`https://v2board.local${normalized}`);

  if (normalized !== current && matchesUserRoute(url.pathname)) throw redirect(normalized);
  throw redirect(getUserRouteFallback());
}

/** Keep `/` as an explicit session-aware entry; no operator-rendered HTML
 * configuration exists in the native runtime. */
export function rootUserRouteLoader(): never {
  throw redirect(getUserRouteFallback());
}

// Auth is guarded in two layers and BOTH are load-bearing:
//   1. This route loader gates ENTRY. On navigation it redirects an
//      unauthenticated request to /login before the page renders, and warms the
//      user/info query for authenticated entries. Loaders only run on
//      navigation, so this layer alone cannot react to a logout that happens
//      while the user is already on a guarded page.
//   2. <RequireAuth> (require-auth.tsx) gates the LIVE session. It subscribes to
//      the auth store via useAuthData(), so a logout() (or token2Login) while a
//      guarded page is mounted re-renders and <Navigate>s to /login.
// Behavioral coverage: createRequireUserLoader tests in App.test.tsx (layer 1)
// and require-auth.test.tsx (layer 2).
export function createRequireUserLoader(queryClient: QueryClient) {
  return async ({ request }: LoaderFunctionArgs) => {
    const current = getRequestRoutePath(request);

    if (!getAuthData()) {
      throw redirect(buildLoginRedirect(current));
    }

    try {
      await queryClient.ensureQueryData(userQueryOptions.info());
    } catch (error) {
      // A 403 clears auth in the API layer; convert that teardown into a router
      // redirect. Other failures belong to the route error boundary—never invent
      // an empty user object that looks like valid authenticated server state.
      if (!getAuthData()) throw redirect('/login');
      throw error;
    }
    return null;
  };
}

interface LoginProbeConsumer {
  signal: AbortSignal;
}

function isAbortedQuery(error: unknown): boolean {
  return (
    error instanceof CancelledError ||
    (typeof error === 'object' && error !== null && 'name' in error && error.name === 'AbortError')
  );
}

const activeLoginProbeConsumers = new WeakMap<QueryClient, Set<LoginProbeConsumer>>();

function trackLoginProbe(queryClient: QueryClient, signal: AbortSignal): () => void {
  const active = activeLoginProbeConsumers.get(queryClient) ?? new Set<LoginProbeConsumer>();
  activeLoginProbeConsumers.set(queryClient, active);
  const consumer = { signal };
  active.add(consumer);

  const onAbort = () => {
    active.delete(consumer);
    // Keep a shared probe alive while another concurrent loader still needs it;
    // cancel only when every consumer has gone away.
    if (active.size === 0) {
      activeLoginProbeConsumers.delete(queryClient);
      void queryClient.cancelQueries({ queryKey: userKeys.checkLogin, exact: true });
    }
  };
  signal.addEventListener('abort', onAbort, { once: true });

  return () => {
    signal.removeEventListener('abort', onAbort);
    active.delete(consumer);
    if (active.size === 0) activeLoginProbeConsumers.delete(queryClient);
  };
}

/**
 * Resolve an existing session before the login component renders. TanStack
 * Query owns the request lifecycle and AbortSignal; repeated/concurrent loader
 * calls join the same checkLogin promise instead of issuing duplicate probes.
 * A verify handoff is deliberately excluded: token2Login mints a new session
 * and must never race a stale-token check.
 */
export function createLoginLoader(queryClient: QueryClient) {
  return async ({ request }: LoaderFunctionArgs) => {
    const routeUrl = new URL(getRequestRoutePath(request), 'https://v2board.local');
    if (request.signal.aborted || routeUrl.searchParams.has('verify') || !getAuthData()) {
      return null;
    }

    let session: CheckLoginResult;
    const stopTracking = trackLoginProbe(queryClient, request.signal);
    try {
      session = await queryClient.fetchQuery(userQueryOptions.checkLogin());
    } catch (error) {
      // An abandoned navigation owns no UI and should not poison the next one.
      // Real query failures belong to the route error boundary; silently showing
      // a credential form would misrepresent an unavailable session check as a
      // verified logged-out state.
      if (request.signal.aborted || isAbortedQuery(error)) return null;
      throw error;
    } finally {
      stopTracking();
    }

    if (request.signal.aborted) return null;

    if (!session.is_login) {
      setAuthData(null);
      return null;
    }

    // The old effect navigated immediately and warmed user/info in parallel.
    // Keep that non-blocking contract while deduping against the destination.
    void queryClient.prefetchQuery(userQueryOptions.info());
    throw redirect(normalizeLoginRedirectTarget(routeUrl.searchParams.get('redirect')));
  };
}

// Warms the dashboard's own queries while its lazy chunk is still downloading,
// so the page paints from cache instead of firing them only after mount. It runs
// in parallel with the AppLayout requireUser loader, so it guards auth itself —
// an unauthenticated entry must not fire user-scoped queries (requireUser issues
// the redirect). Fire-and-forget on purpose: prefetching must never delay the
// route, and the page's own useQuery hooks dedupe onto these in-flight requests
// on mount. Dashboard has no pinned fetch-order contract, so warming these four
// in parallel is contract-safe.
export function createDashboardPrefetchLoader(queryClient: QueryClient) {
  return (): null => {
    if (!getAuthData()) return null;
    // prefetchQuery is the canonical detached warm-up API: it retains query
    // failures in cache for each surface's ErrorState and never rejects/toasts.
    void queryClient.prefetchQuery(userQueryOptions.subscribe());
    void queryClient.prefetchQuery(userQueryOptions.stat());
    void queryClient.prefetchQuery(userQueryOptions.notices());
    void queryClient.prefetchQuery(userQueryOptions.commConfig());
    return null;
  };
}

// Keep initial loader and lazy-chunk work visible and accessible while the data
// router hydrates. Mirrors AppLayout's role=status spinner fallback.
function RouteHydrateFallback() {
  const { t } = useTranslation();
  return (
    <div
      role="status"
      className="flex min-h-screen items-center justify-center bg-background"
    >
      <Spinner className="size-6" />
      <span className="sr-only">{t($ => $.common.loading)}</span>
    </div>
  );
}

function pageRoute(path: UserPageRoutePath, loader?: RouteObject['loader']): RouteObject {
  return {
    path,
    lazy: lazyPage(path),
    errorElement: <RouteErrorFallback />,
    ...(loader ? { loader } : {}),
  };
}

export function createUserRoutes(queryClient: QueryClient): RouteObject[] {
  const requireUser = createRequireUserLoader(queryClient);
  const loginLoader = createLoginLoader(queryClient);
  const prefetchDashboard = createDashboardPrefetchLoader(queryClient);

  return [
    {
      id: 'user-root',
      loader: normalizeUserRouteLoader,
      element: <RouteBoundaryOutlet />,
      errorElement: <RouteErrorFallback />,
      hydrateFallbackElement: <RouteHydrateFallback />,
      children: [
        {
          path: '/',
          loader: rootUserRouteLoader,
          errorElement: <RouteErrorFallback />,
        },
        {
          element: <GuestLayout />,
          errorElement: <RouteErrorFallback />,
          children: USER_GUEST_ROUTE_PATHS.map((path) =>
            pageRoute(path, path === '/login' ? loginLoader : undefined),
          ),
        },
        {
          loader: requireUser,
          element: (
            <RequireAuth>
              <AppLayout />
            </RequireAuth>
          ),
          errorElement: <RouteErrorFallback />,
          children: USER_APP_LAYOUT_ROUTE_PATHS.map((path) =>
            path === '/dashboard' ? pageRoute(path, prefetchDashboard) : pageRoute(path),
          ),
        },
        {
          path: '/ticket/:ticket_id',
          loader: requireUser,
          element: (
            <RequireAuth>
              <RouteBoundaryOutlet />
            </RequireAuth>
          ),
          errorElement: <RouteErrorFallback />,
          children: [
            {
              index: true,
              lazy: lazyPage('/ticket/:ticket_id'),
              errorElement: <RouteErrorFallback />,
            },
          ],
        },
        {
          path: '*',
          loader: unknownUserRouteLoader,
          errorElement: <RouteErrorFallback />,
        },
      ],
    },
  ];
}

export function createUserRouter(queryClient: QueryClient) {
  return createHashRouter(createUserRoutes(queryClient));
}
