import { useEffect, type ReactNode } from 'react';
import { App as AntdApp } from 'antd';
import { Navigate, Route, Routes, matchPath, useLocation, useNavigate } from 'react-router-dom';
import { getNormalizedLegacyHashPath } from '@v2board/config';
import { AdminLayout } from '@/components/layout/admin-layout';
import { bindNotificationApi } from '@/lib/api';
import { getAuthData } from '@/lib/auth';
import LoginPage from '@/pages/login';
import DashboardPage from '@/pages/dashboard';
import UsersPage from '@/pages/users';
import OrdersPage from '@/pages/orders';
import PlansPage from '@/pages/plans';
import ServersPage from '@/pages/servers';
import TicketsPage from '@/pages/tickets';
import PaymentsPage from '@/pages/payments';
import CouponsPage from '@/pages/coupons';
import KnowledgePage from '@/pages/knowledge';
import NoticesPage from '@/pages/notices';
import SystemPage from '@/pages/system';
import ConfigPage from '@/pages/config';
import { RouteBoundaryElement } from '@/components/route-error-boundary';

export const ADMIN_LEGACY_ROUTE_PATHS = [
  '/config/payment',
  '/config/system',
  '/config/theme',
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

type AdminLegacyRoutePath = (typeof ADMIN_LEGACY_ROUTE_PATHS)[number];

export const ADMIN_STANDALONE_ROUTE_PATHS = [
  '/ticket/:ticket_id',
] as const satisfies readonly AdminLegacyRoutePath[];

export const ADMIN_LAYOUT_ROUTE_PATHS = [
  '/config/payment',
  '/config/system',
  '/config/theme',
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
] as const satisfies readonly AdminLegacyRoutePath[];

const ADMIN_LEGACY_ROUTE_OPTIONS = {
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  nestedPrefixes: ADMIN_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login'],
  routes: ADMIN_LEGACY_ROUTE_PATHS,
} as const;

function matchesAdminLegacyRoute(pathname: string): boolean {
  return ADMIN_LEGACY_ROUTE_PATHS.some((path) => matchPath({ path, end: true }, pathname));
}

function getAdminRouteFallback(): string {
  return getAuthData()
    ? ADMIN_LEGACY_ROUTE_OPTIONS.authenticatedFallback
    : ADMIN_LEGACY_ROUTE_OPTIONS.guestFallback;
}

function RootRedirect() {
  const navigate = useNavigate();

  useEffect(() => {
    navigate('/login');
  }, [navigate]);

  return <div />;
}

function LegacyUnknownRouteRedirect() {
  const location = useLocation();
  const current = `${location.pathname}${location.search}`;
  const normalized = getNormalizedLegacyHashPath(current, ADMIN_LEGACY_ROUTE_OPTIONS);

  return <Navigate to={normalized} replace />;
}

const ADMIN_ROUTE_ELEMENTS: Record<AdminLegacyRoutePath, ReactNode> = {
  '/config/payment': <PaymentsPage />,
  '/config/system': <ConfigPage />,
  '/config/theme': <ConfigPage />,
  '/coupon': <CouponsPage />,
  '/giftcard': <CouponsPage />,
  '/dashboard': <DashboardPage />,
  '/': <RootRedirect />,
  '/knowledge': <KnowledgePage />,
  '/login': <LoginPage />,
  '/notice': <NoticesPage />,
  '/order': <OrdersPage />,
  '/plan': <PlansPage />,
  '/queue': <SystemPage />,
  '/server/group': <ServersPage />,
  '/server/manage': <ServersPage />,
  '/server/route': <ServersPage />,
  '/ticket/:ticket_id': <TicketsPage />,
  '/ticket': <TicketsPage />,
  '/user': <UsersPage />,
};

export default function App() {
  const { notification } = AntdApp.useApp();
  const location = useLocation();
  const current = `${location.pathname}${location.search}`;
  const normalized = getNormalizedLegacyHashPath(current, ADMIN_LEGACY_ROUTE_OPTIONS);

  useEffect(() => bindNotificationApi(notification), [notification]);

  if (normalized !== current) return <Navigate to={normalized} replace />;
  if (!matchesAdminLegacyRoute(location.pathname)) {
    return <Navigate to={getAdminRouteFallback()} replace />;
  }

  return (
    <Routes>
      <Route
        path="/login"
        element={<RouteBoundaryElement>{ADMIN_ROUTE_ELEMENTS['/login']}</RouteBoundaryElement>}
      />
      <Route
        path="/"
        element={<RouteBoundaryElement>{ADMIN_ROUTE_ELEMENTS['/']}</RouteBoundaryElement>}
      />
      <Route
        path="/ticket/:ticket_id"
        element={
          <RouteBoundaryElement>{ADMIN_ROUTE_ELEMENTS['/ticket/:ticket_id']}</RouteBoundaryElement>
        }
      />
      <Route element={<AdminLayout />}>
        {ADMIN_LAYOUT_ROUTE_PATHS.map((path) => (
          <Route key={path} path={path} element={ADMIN_ROUTE_ELEMENTS[path]} />
        ))}
      </Route>
      <Route
        path="*"
        element={
          <RouteBoundaryElement>
            <LegacyUnknownRouteRedirect />
          </RouteBoundaryElement>
        }
      />
    </Routes>
  );
}
