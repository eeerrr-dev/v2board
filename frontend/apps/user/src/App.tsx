import { Route, Routes, useLocation } from 'react-router-dom';
import type { ReactNode } from 'react';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';
import { RouteErrorBoundary } from '@/components/route-error-boundary';
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
  '/dashboard': (
    <RouteErrorBoundary>
      <DashboardPage />
    </RouteErrorBoundary>
  ),
  '/forgetpassword': (
    <RouteErrorBoundary>
      <ForgetPage />
    </RouteErrorBoundary>
  ),
  '/': (
    <RouteErrorBoundary>
      <HomePage />
    </RouteErrorBoundary>
  ),
  '/invite': (
    <RouteErrorBoundary>
      <InvitePage />
    </RouteErrorBoundary>
  ),
  '/knowledge': (
    <RouteErrorBoundary>
      <KnowledgePage />
    </RouteErrorBoundary>
  ),
  '/login': (
    <RouteErrorBoundary>
      <LoginPage />
    </RouteErrorBoundary>
  ),
  '/node': (
    <RouteErrorBoundary>
      <NodePage />
    </RouteErrorBoundary>
  ),
  '/order/:trade_no': (
    <RouteErrorBoundary>
      <OrderDetailPage />
    </RouteErrorBoundary>
  ),
  '/order': (
    <RouteErrorBoundary>
      <OrdersPage />
    </RouteErrorBoundary>
  ),
  '/plan/:plan_id': (
    <RouteErrorBoundary>
      <PlanCheckoutPage />
    </RouteErrorBoundary>
  ),
  '/plan': (
    <RouteErrorBoundary>
      <PlansPage />
    </RouteErrorBoundary>
  ),
  '/profile': (
    <RouteErrorBoundary>
      <ProfilePage />
    </RouteErrorBoundary>
  ),
  '/register': (
    <RouteErrorBoundary>
      <RegisterPage />
    </RouteErrorBoundary>
  ),
  '/ticket/:ticket_id': (
    <RouteErrorBoundary>
      <TicketDetailPage />
    </RouteErrorBoundary>
  ),
  '/ticket': (
    <RouteErrorBoundary>
      <TicketsPage />
    </RouteErrorBoundary>
  ),
  '/traffic': (
    <RouteErrorBoundary>
      <TrafficPage />
    </RouteErrorBoundary>
  ),
};

const USER_GUEST_ROUTE_PATHS = ['/login', '/register', '/forgetpassword'] as const;

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

function routeComponentKey(pathname: string): string {
  if (/^\/order\/[^/]+$/.test(pathname)) return '/order/:trade_no';
  if (/^\/plan\/[^/]+$/.test(pathname)) return '/plan/:plan_id';
  if (/^\/ticket\/[^/]+$/.test(pathname)) return '/ticket/:ticket_id';
  return pathname;
}

function KeyedGuestLayout() {
  const location = useLocation();
  return <GuestLayout key={routeComponentKey(location.pathname)} />;
}

function KeyedAppLayout() {
  const location = useLocation();
  return <AppLayout key={routeComponentKey(location.pathname)} />;
}

export default function App() {
  return (
    <Routes>
      <Route path="/" element={USER_ROUTE_ELEMENTS['/']} />
      <Route element={<KeyedGuestLayout />}>
        {USER_GUEST_ROUTE_PATHS.map((path) => (
          <Route key={path} path={path} element={USER_ROUTE_ELEMENTS[path]} />
        ))}
      </Route>
      <Route element={<KeyedAppLayout />}>
        {USER_APP_LAYOUT_ROUTE_PATHS.map((path) => (
          <Route key={path} path={path} element={USER_ROUTE_ELEMENTS[path]} />
        ))}
      </Route>
      <Route path="/ticket/:ticket_id" element={USER_ROUTE_ELEMENTS['/ticket/:ticket_id']} />
      <Route path="*" element={USER_ROUTE_ELEMENTS['/']} />
    </Routes>
  );
}
