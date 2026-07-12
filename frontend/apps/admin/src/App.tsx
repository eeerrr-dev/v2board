import { CancelledError, type QueryClient } from '@tanstack/react-query';
import { type ComponentType } from 'react';
import {
  createHashRouter,
  matchPath,
  redirect,
  type LoaderFunctionArgs,
  type RouteObject,
} from 'react-router';
import { getNormalizedHashPath } from '@v2board/config';
import type { CheckLoginResult } from '@v2board/types';
import { AdminLayout } from '@/components/layout/admin-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet, RouteErrorFallback } from '@/components/route-error-boundary';
import { Spinner } from '@/components/ui/spinner';
import { buildLoginRedirect, getAuthData, logout, type AdminLoginLoaderData } from '@/lib/auth';
import { adminSessionQueryOptions } from '@/lib/session-queries';

export const ADMIN_ROUTE_PATHS = [
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

export const ADMIN_HASH_ROUTE_OPTIONS = {
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  nestedPrefixes: ADMIN_ROUTE_PATHS,
  publicRoutes: ['/', '/login'],
  routes: ADMIN_ROUTE_PATHS,
} as const;

type LazyRouteModule = Promise<{ default: ComponentType }>;

const ADMIN_ROUTE_MODULES: Record<AdminRoutePath, () => LazyRouteModule> = {
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

function getRequestRoutePath(request: Request): string {
  const url = new URL(request.url);
  if (url.hash.startsWith('#/')) return url.hash.slice(1);
  return `${url.pathname}${url.search}`;
}

function matchesAdminRoute(pathname: string): boolean {
  return ADMIN_ROUTE_PATHS.some((path) => matchPath({ path, end: true }, pathname));
}

export function normalizeAdminRouteLoader({ request }: LoaderFunctionArgs) {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedHashPath(current, ADMIN_HASH_ROUTE_OPTIONS);
  if (normalized !== current) throw redirect(normalized);
  return null;
}

export function rootAdminRouteLoader(): never {
  throw redirect(getAuthData() ? '/dashboard' : '/login');
}

export function unknownAdminRouteLoader({ request }: LoaderFunctionArgs): never {
  const current = getRequestRoutePath(request);
  const normalized = getNormalizedHashPath(current, ADMIN_HASH_ROUTE_OPTIONS);
  const url = new URL(`https://v2board.local${normalized}`);
  if (normalized !== current && matchesAdminRoute(url.pathname)) throw redirect(normalized);
  throw redirect(getAuthData() ? '/dashboard' : '/login');
}

export function createRequireAdminLoader(queryClient: QueryClient) {
  return async ({ request }: LoaderFunctionArgs) => {
    const current = getRequestRoutePath(request);
    if (!getAuthData()) throw redirect(buildLoginRedirect(current));

    const session = await queryClient.ensureQueryData(adminSessionQueryOptions.session());
    if (!session.is_login || !session.is_admin) {
      logout();
      throw redirect('/login');
    }

    // The shell renders the authenticated identity, so it is route data rather
    // than an optional warm-up. A real failure belongs to the route boundary.
    await queryClient.ensureQueryData(adminSessionQueryOptions.userInfo());
    return null;
  };
}

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

    if (!session.is_login || !session.is_admin) {
      logout();
      return data;
    }

    // Resolve the identity before redirecting into the protected shell. The
    // destination loader reads the same fresh query and issues no duplicate.
    await queryClient.ensureQueryData(adminSessionQueryOptions.userInfo());
    throw redirect(data.redirectTarget);
  };
}

function RouteHydrateFallback() {
  return (
    <div
      role="status"
      data-slot="app-loading"
      className="flex min-h-screen items-center justify-center bg-background"
    >
      <Spinner className="size-6" />
      <span className="sr-only">正在加载</span>
    </div>
  );
}

function pageRoute(path: AdminRoutePath): RouteObject {
  return { path, lazy: lazyPage(path), errorElement: <RouteErrorFallback /> };
}

export function createAdminRoutes(queryClient: QueryClient): RouteObject[] {
  const requireAdmin = createRequireAdminLoader(queryClient);
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
          loader: requireAdmin,
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
          loader: requireAdmin,
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
        { path: '*', loader: unknownAdminRouteLoader, errorElement: <RouteErrorFallback /> },
      ],
    },
  ];
}

export function createAdminRouter(queryClient: QueryClient) {
  return createHashRouter(createAdminRoutes(queryClient));
}
