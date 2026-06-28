import { type QueryClient } from '@tanstack/react-query';
import { type ComponentType } from 'react';
import {
  createHashRouter,
  matchPath,
  redirect,
  type LoaderFunctionArgs,
  type RouteObject,
} from 'react-router';
import { getNormalizedLegacyHashPath } from '@v2board/config';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { RouteBoundaryOutlet, RouteErrorFallback } from '@/components/route-error-boundary';
import { getAuthData } from '@/lib/auth';
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

export function createRequireUserLoader(queryClient: QueryClient) {
  return async ({ request }: LoaderFunctionArgs) => {
    const current = getRequestRoutePath(request);

    if (!getAuthData()) {
      throw redirect(`/login?redirect=${encodeURIComponent(current)}`);
    }

    await queryClient.ensureQueryData(userQueryOptions.info()).catch(() => null);
    return null;
  };
}

function pageRoute(path: UserLegacyRoutePath): RouteObject {
  return {
    path,
    lazy: lazyPage(path),
    errorElement: <RouteErrorFallback />,
  };
}

export function createUserRoutes(queryClient: QueryClient): RouteObject[] {
  const requireUser = createRequireUserLoader(queryClient);

  return [
    {
      id: 'user-root',
      loader: normalizeUserRouteLoader,
      element: <RouteBoundaryOutlet />,
      errorElement: <RouteErrorFallback />,
      children: [
        pageRoute('/'),
        {
          element: <GuestLayout />,
          errorElement: <RouteErrorFallback />,
          children: USER_GUEST_ROUTE_PATHS.map(pageRoute),
        },
        {
          loader: requireUser,
          element: (
            <RequireAuth>
              <AppLayout />
            </RequireAuth>
          ),
          errorElement: <RouteErrorFallback />,
          children: USER_APP_LAYOUT_ROUTE_PATHS.map(pageRoute),
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
