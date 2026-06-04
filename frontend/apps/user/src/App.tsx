import { useEffect, type ReactNode } from 'react';
import { Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { getNormalizedLegacyHashPath } from '@v2board/config';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import LoginPage from '@/pages/auth/login';
import RegisterPage from '@/pages/auth/register';
import ForgetPage from '@/pages/auth/forget';
import HomePage from '@/pages/home';
import DashboardPage from '@/pages/dashboard';
import PlansPage from '@/pages/plans';
import PlanCheckoutPage from '@/pages/plans/checkout';
import OrdersPage from '@/pages/orders';
import OrderDetailPage from '@/pages/orders/detail';
import ProfilePage from '@/pages/profile';
import InvitePage from '@/pages/invite';
import TicketsPage from '@/pages/tickets';
import TicketDetailPage from '@/pages/tickets/detail';
import KnowledgePage from '@/pages/knowledge';
import NodePage from '@/pages/node';
import TrafficPage from '@/pages/traffic';
import { RouteBoundaryElement } from '@/components/route-error-boundary';

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

const USER_ROUTE_ELEMENTS: Record<UserLegacyRoutePath, ReactNode> = {
  '/dashboard': <DashboardPage />,
  '/forgetpassword': <ForgetPage />,
  '/': <HomePage />,
  '/invite': <InvitePage />,
  '/knowledge': <KnowledgePage />,
  '/login': <LoginPage />,
  '/node': <NodePage />,
  '/order/:trade_no': <OrderDetailPage />,
  '/order': <OrdersPage />,
  '/plan/:plan_id': <PlanCheckoutPage />,
  '/plan': <PlansPage />,
  '/profile': <ProfilePage />,
  '/register': <RegisterPage />,
  '/ticket/:ticket_id': <TicketDetailPage />,
  '/ticket': <TicketsPage />,
  '/traffic': <TrafficPage />,
};

const USER_GUEST_ROUTE_PATHS = ['/login', '/register', '/forgetpassword'] as const;

const USER_LEGACY_ROUTE_OPTIONS = {
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  nestedPrefixes: USER_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_LEGACY_ROUTE_PATHS,
} as const;

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

function LegacyUnknownRouteRedirect() {
  const location = useLocation();
  const navigate = useNavigate();
  const current = `${location.pathname}${location.search}`;
  const normalized = getNormalizedLegacyHashPath(current, USER_LEGACY_ROUTE_OPTIONS);

  useEffect(() => {
    if (normalized !== current) navigate(normalized, { replace: true });
  }, [current, navigate, normalized]);

  return (
    <div className="content content-full text-center">
      <div className="spinner-grow text-primary" role="status">
        <span className="sr-only">Loading...</span>
      </div>
    </div>
  );
}

export default function App() {
  return (
    <Routes>
      <Route
        path="/"
        element={<RouteBoundaryElement>{USER_ROUTE_ELEMENTS['/']}</RouteBoundaryElement>}
      />
      <Route element={<GuestLayout />}>
        {USER_GUEST_ROUTE_PATHS.map((path) => (
          <Route key={path} path={path} element={USER_ROUTE_ELEMENTS[path]} />
        ))}
      </Route>
      <Route element={<AppLayout />}>
        {USER_APP_LAYOUT_ROUTE_PATHS.map((path) => (
          <Route key={path} path={path} element={USER_ROUTE_ELEMENTS[path]} />
        ))}
      </Route>
      <Route
        path="/ticket/:ticket_id"
        element={
          <RouteBoundaryElement>{USER_ROUTE_ELEMENTS['/ticket/:ticket_id']}</RouteBoundaryElement>
        }
      />
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
