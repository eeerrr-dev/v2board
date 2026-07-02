import { type QueryClient } from '@tanstack/react-query';
import { type ComponentType } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createHashRouter,
  matchPath,
  redirect,
  type LoaderFunctionArgs,
  type RouteObject,
} from 'react-router';
import { getNormalizedLegacyHashPath } from '@v2board/config';
import type { UserInfo } from '@v2board/types';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet, RouteErrorFallback } from '@/components/route-error-boundary';
import { Spinner } from '@/components/ui/spinner';
import { buildLoginRedirect, getAuthData } from '@/lib/auth';
import { userQueryOptions } from '@/lib/queries';

export const USER_LEGACY_ROUTE_PATHS = [
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

type UserLegacyRoutePath = (typeof USER_LEGACY_ROUTE_PATHS)[number];

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

export const USER_LEGACY_ROUTE_OPTIONS = {
  authenticatedFallback: '/dashboard',
  authenticatedPublicFallbackRoutes: [],
  guestFallback: '/login',
  nestedPrefixes: USER_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_LEGACY_ROUTE_PATHS,
} as const;

type LazyRouteModule = Promise<{ default: ComponentType }>;

const USER_ROUTE_MODULES: Record<UserLegacyRoutePath, () => LazyRouteModule> = {
  '/dashboard': () => import('@/pages/dashboard'),
  '/forgetpassword': () => import('@/pages/auth/forget'),
  '/': () => import('@/pages/home'),
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

function lazyPage(path: UserLegacyRoutePath): RouteObject['lazy'] {
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

function matchesUserLegacyRoute(pathname: string): boolean {
  return USER_LEGACY_ROUTE_PATHS.some((path) => matchPath({ path, end: true }, pathname));
}

function getUserRouteFallback(): string {
  return getAuthData()
    ? USER_LEGACY_ROUTE_OPTIONS.authenticatedFallback
    : USER_LEGACY_ROUTE_OPTIONS.guestFallback;
}

export function normalizeUserRouteLoader({ request }: LoaderFunctionArgs) {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedLegacyHashPath(current, USER_LEGACY_ROUTE_OPTIONS);

  if (normalized !== current) throw redirect(normalized);
  return null;
}

export function unknownUserRouteLoader({ request }: LoaderFunctionArgs) {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedLegacyHashPath(current, USER_LEGACY_ROUTE_OPTIONS);
  const url = new URL(`https://v2board.local${normalized}`);

  if (normalized !== current && matchesUserLegacyRoute(url.pathname)) throw redirect(normalized);
  throw redirect(getUserRouteFallback());
}

// The legacy empty user record: the packaged frontend initialized its user
// state to an empty object and kept rendering the app shell when /user/info
// failed without a session teardown (the oracle's user-auth-401-no-redirect
// scenario keeps the dashboard mounted on an HTTP 401). AppLayout reads the
// info query via useSuspenseQuery, which would re-throw a cached fetch error
// to the route errorElement and replace the whole shell — so the require-user
// loader seeds this record instead and page-level queries surface their own
// failures. Page observers refetch the real record as soon as one succeeds.
const EMPTY_USER_INFO: UserInfo = {
  email: '',
  transfer_enable: 0,
  device_limit: null,
  last_login_at: null,
  created_at: 0,
  banned: 0,
  auto_renewal: 0,
  remind_expire: 0,
  remind_traffic: 0,
  expired_at: null,
  balance: 0,
  commission_balance: 0,
  plan_id: null,
  discount: null,
  commission_rate: null,
  telegram_id: null,
  uuid: '',
  avatar_url: '',
};

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
    } catch {
      // A failed /user/info must not unmount the app shell (see EMPTY_USER_INFO
      // above). Only a 403 tears the session down: redirectToLegacyLogin has
      // already cleared the token by the time that rejection propagates here,
      // so the getAuthData() check skips the seed and the auth gates own the
      // redirect. A query that already holds data keeps it.
      if (
        getAuthData() &&
        queryClient.getQueryData(userQueryOptions.info().queryKey) === undefined
      ) {
        queryClient.setQueryData(userQueryOptions.info().queryKey, EMPTY_USER_INFO);
      }
    }
    return null;
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
    void queryClient.ensureQueryData(userQueryOptions.subscribe()).catch(() => null);
    void queryClient.ensureQueryData(userQueryOptions.stat()).catch(() => null);
    void queryClient.ensureQueryData(userQueryOptions.notices()).catch(() => null);
    void queryClient.ensureQueryData(userQueryOptions.commConfig()).catch(() => null);
    return null;
  };
}

// Initial-hydration fallback: while the root matches' loaders/lazy chunks are
// still pending on first load (e.g. requireUser awaiting a slow /user/info),
// the data router would otherwise render nothing and leave #root empty — the
// exact DOM shape the retired production white-screen watchdog keyed on.
// Mirrors AppLayout's role=status spinner fallback.
function RouteHydrateFallback() {
  const { t } = useTranslation();
  return (
    <div
      role="status"
      className="v2board-island flex min-h-screen items-center justify-center bg-background"
    >
      <Spinner className="size-6" />
      <span className="sr-only">{t('common.loading')}</span>
    </div>
  );
}

function pageRoute(path: UserLegacyRoutePath, loader?: RouteObject['loader']): RouteObject {
  return {
    path,
    lazy: lazyPage(path),
    errorElement: <RouteErrorFallback />,
    ...(loader ? { loader } : {}),
  };
}

export function createUserRoutes(queryClient: QueryClient): RouteObject[] {
  const requireUser = createRequireUserLoader(queryClient);
  const prefetchDashboard = createDashboardPrefetchLoader(queryClient);

  return [
    {
      id: 'user-root',
      loader: normalizeUserRouteLoader,
      element: <RouteBoundaryOutlet />,
      errorElement: <RouteErrorFallback />,
      hydrateFallbackElement: <RouteHydrateFallback />,
      children: [
        pageRoute('/'),
        {
          element: <GuestLayout />,
          errorElement: <RouteErrorFallback />,
          children: USER_GUEST_ROUTE_PATHS.map((path) => pageRoute(path)),
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
