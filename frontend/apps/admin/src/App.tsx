import { CancelledError, type QueryClient } from '@tanstack/react-query';
import { type ComponentType } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createBrowserRouter,
  matchPath,
  redirect,
  type LoaderFunctionArgs,
  type MiddlewareFunction,
  type RouteObject,
} from 'react-router';
import { getNormalizedRoutePath, stripBasePath } from '@v2board/config';
import type { CheckLoginResult } from '@v2board/types';
import { AdminLayout } from '@/components/layout/admin-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet, RouteErrorFallback } from '@/components/route-error-boundary';
import { Spinner } from '@v2board/ui/spinner';
import {
  AUTH_KEY,
  buildLoginRedirect,
  getAuthData,
  logout,
  type AdminLoginLoaderData,
} from '@/lib/auth';
import { getAdminBasename } from '@/lib/runtime-config';
import { canEnterAdminNamespace, firstAllowedRoute, sessionAllowsRoute } from '@/lib/permissions';
import { adminSessionQueryOptions } from '@/lib/session-queries';

export const ADMIN_ROUTE_PATHS = [
  '/audit',
  '/config/payment',
  '/config/system',
  '/coupon',
  '/giftcard',
  '/dashboard',
  '/',
  '/knowledge',
  '/login',
  '/notice',
  '/order',
  '/plan',
  '/queue',
  '/server/group',
  '/server/manage',
  '/server/route',
  '/ticket/:ticket_id',
  '/ticket',
  '/user',
] as const;

type AdminRoutePath = (typeof ADMIN_ROUTE_PATHS)[number];

export const ADMIN_STANDALONE_ROUTE_PATHS = [
  '/ticket/:ticket_id',
] as const satisfies readonly AdminRoutePath[];

export const ADMIN_LAYOUT_ROUTE_PATHS = [
  '/audit',
  '/config/payment',
  '/config/system',
  '/coupon',
  '/giftcard',
  '/dashboard',
  '/knowledge',
  '/notice',
  '/order',
  '/plan',
  '/queue',
  '/server/group',
  '/server/manage',
  '/server/route',
  '/ticket',
  '/user',
] as const satisfies readonly AdminRoutePath[];

function isAbortedQuery(error: unknown): boolean {
  return (
    error instanceof CancelledError ||
    (typeof error === 'object' && error !== null && 'name' in error && error.name === 'AbortError')
  );
}

export const ADMIN_ROUTE_GUARD_OPTIONS = {
  matchRoute: (route: string, path: string, end: boolean) => matchPath({ path: route, end }, path),
  authStorageKey: AUTH_KEY,
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  nestedPrefixes: ADMIN_ROUTE_PATHS,
  publicRoutes: ['/', '/login'],
  routes: ADMIN_ROUTE_PATHS,
} as const;

type LazyRouteModule = Promise<{ default: ComponentType }>;

const ADMIN_ROUTE_MODULES: Record<AdminRoutePath, () => LazyRouteModule> = {
  '/audit': () => import('@/pages/audit'),
  '/config/payment': () => import('@/pages/payments'),
  '/config/system': () => import('@/pages/config'),
  '/coupon': () => import('@/pages/coupons'),
  '/giftcard': () => import('@/pages/coupons'),
  '/dashboard': () => import('@/pages/dashboard'),
  '/': () => import('@/pages/login'),
  '/knowledge': () => import('@/pages/knowledge'),
  '/login': () => import('@/pages/login'),
  '/notice': () => import('@/pages/notices'),
  '/order': () => import('@/pages/orders'),
  '/plan': () => import('@/pages/plans'),
  '/queue': () => import('@/pages/system'),
  '/server/group': () => import('@/pages/servers'),
  '/server/manage': () => import('@/pages/servers'),
  '/server/route': () => import('@/pages/servers'),
  '/ticket/:ticket_id': () => import('@/pages/tickets'),
  '/ticket': () => import('@/pages/tickets'),
  '/user': () => import('@/pages/users'),
};

function lazyPage(path: AdminRoutePath): RouteObject['lazy'] {
  return async () => {
    const module = await ADMIN_ROUTE_MODULES[path]();
    return { Component: module.default };
  };
}

// History routing (docs/api-dialect.md §10.1): the admin router mounts under
// the dynamic `/{admin_path}` base, so loader request URLs carry the basename
// while route matching, guards, and thrown `redirect()` targets stay
// app-relative (react-router prepends the basename on redirect navigation).
function getRequestRoutePath(request: Request): string {
  const url = new URL(request.url);
  return `${stripBasePath(url.pathname, getAdminBasename())}${url.search}`;
}

function matchesAdminRoute(pathname: string): boolean {
  return ADMIN_ROUTE_PATHS.some((path) => matchPath({ path, end: true }, pathname));
}

export function normalizeAdminRouteLoader({ request }: LoaderFunctionArgs) {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedRoutePath(current, ADMIN_ROUTE_GUARD_OPTIONS);
  if (normalized !== current) throw redirect(normalized);
  return null;
}

export function rootAdminRouteLoader(): never {
  throw redirect(getAuthData() ? '/dashboard' : '/login');
}

export function unknownAdminRouteLoader({ request }: LoaderFunctionArgs): never {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedRoutePath(current, ADMIN_ROUTE_GUARD_OPTIONS);
  const url = new URL(`https://v2board.local${normalized}`);
  if (normalized !== current && matchesAdminRoute(url.pathname)) throw redirect(normalized);
  throw redirect(getAuthData() ? '/dashboard' : '/login');
}

// Route middleware on the protected subtree: it runs ahead of the loaders of
// every navigation inside that branch (not just on entry, unlike the parent
// loader it replaced). <RequireAuth> stays as the second layer, reacting to a
// logout while a guarded page is already mounted.
export function createRequireAdminMiddleware(queryClient: QueryClient): MiddlewareFunction {
  return async ({ request }) => {
    const current = getRequestRoutePath(request);
    if (!getAuthData()) throw redirect(buildLoginRedirect(current));

    const session = await queryClient.ensureQueryData(adminSessionQueryOptions.session());
    // §6.12: full admins and granted staff enter; anyone else never does.
    if (!session.is_login || !canEnterAdminNamespace(session)) {
      logout();
      throw redirect('/login');
    }
    // A staff session may only open routes its grants can read — send
    // anything else to its first readable destination instead of a 403 wall.
    const pathname = new URL(request.url).pathname;
    if (!sessionAllowsRoute(session, stripBasePath(pathname, getAdminBasename()))) {
      throw redirect(firstAllowedRoute(session));
    }

    // The shell renders the authenticated identity, so it is route data rather
    // than an optional warm-up. A real failure belongs to the route boundary.
    await queryClient.ensureQueryData(adminSessionQueryOptions.userInfo());
  };
}

// Stricter than the user app's normalizeLoginRedirectTarget on purpose: the
// user variant repairs bare paths (`order` -> `/order`) because the backend
// emails bare route names into `redirect` (sessions.rs login_redirect_url,
// getQuickLoginUrl). No external party deep-links into admin, so anything but
// a clean absolute internal path falls back to /dashboard. Do not unify.
function normalizeLoginRedirectTarget(value: string | null): string {
  if (!value) return '/dashboard';
  const normalized = value.trim().replace(/\\/g, '/');
  if (
    !normalized.startsWith('/') ||
    normalized.startsWith('//') ||
    hasControlCharacter(normalized)
  ) {
    return '/dashboard';
  }
  return normalized;
}

function hasControlCharacter(value: string): boolean {
  for (const character of value) {
    const code = character.charCodeAt(0);
    if (code <= 0x1f || code === 0x7f) return true;
  }
  return false;
}

function getLoginLoaderData(request: Request): AdminLoginLoaderData {
  try {
    const routeUrl = new URL(getRequestRoutePath(request), 'https://v2board.local');
    return {
      redirectTarget: normalizeLoginRedirectTarget(routeUrl.searchParams.get('redirect')),
    };
  } catch {
    return { redirectTarget: '/dashboard' };
  }
}

export function createAdminLoginLoader(queryClient: QueryClient) {
  return async ({ request }: LoaderFunctionArgs): Promise<AdminLoginLoaderData> => {
    const data = getLoginLoaderData(request);
    if (!getAuthData()) return data;

    let session: CheckLoginResult;
    try {
      session = await queryClient.ensureQueryData(adminSessionQueryOptions.session());
    } catch (error) {
      // Cancellation has no screen to report into. Every real query failure is
      // routed to the nearest route boundary instead of being disguised as a
      // successful logged-out probe.
      if (request.signal.aborted || isAbortedQuery(error)) return data;
      throw error;
    }

    if (!session.is_login || !canEnterAdminNamespace(session)) {
      logout();
      return data;
    }

    // Resolve the identity before redirecting into the protected shell. The
    // destination loader reads the same fresh query and issues no duplicate.
    await queryClient.ensureQueryData(adminSessionQueryOptions.userInfo());
    const targetPath = new URL(data.redirectTarget, 'https://v2board.local').pathname;
    throw redirect(
      sessionAllowsRoute(session, targetPath) ? data.redirectTarget : firstAllowedRoute(session),
    );
  };
}

function RouteHydrateFallback() {
  const { t } = useTranslation();
  return (
    <div
      role="status"
      data-slot="app-loading"
      className="flex min-h-screen items-center justify-center bg-background"
    >
      <Spinner className="size-6" />
      <span className="sr-only">{t(($) => $.admin.nav.loading)}</span>
    </div>
  );
}

function pageRoute(path: AdminRoutePath): RouteObject {
  return { path, lazy: lazyPage(path), errorElement: <RouteErrorFallback /> };
}

export function createAdminRoutes(queryClient: QueryClient): RouteObject[] {
  const requireAdmin = createRequireAdminMiddleware(queryClient);
  const loginLoader = createAdminLoginLoader(queryClient);

  return [
    {
      id: 'admin-root',
      loader: normalizeAdminRouteLoader,
      element: <RouteBoundaryOutlet />,
      errorElement: <RouteErrorFallback />,
      hydrateFallbackElement: <RouteHydrateFallback />,
      children: [
        { path: '/', loader: rootAdminRouteLoader },
        { ...pageRoute('/login'), loader: loginLoader },
        {
          // One pathless gate for every protected branch: the middleware runs
          // ahead of the loaders of each navigation in this subtree.
          middleware: [requireAdmin],
          errorElement: <RouteErrorFallback />,
          children: [
            {
              element: (
                <RequireAuth>
                  <AdminLayout />
                </RequireAuth>
              ),
              errorElement: <RouteErrorFallback />,
              children: ADMIN_LAYOUT_ROUTE_PATHS.map(pageRoute),
            },
            {
              path: '/ticket/:ticket_id',
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
          ],
        },
        { path: '*', loader: unknownAdminRouteLoader, errorElement: <RouteErrorFallback /> },
      ],
    },
  ];
}

export function createAdminRouter(queryClient: QueryClient) {
  return createBrowserRouter(createAdminRoutes(queryClient), {
    basename: getAdminBasename(),
  });
}
