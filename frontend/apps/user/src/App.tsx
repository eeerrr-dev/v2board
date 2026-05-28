import { Suspense, lazy } from 'react';
import { Route, Routes } from 'react-router-dom';
import { AppLayout } from '@/components/layout/app-layout';
import { GuestLayout } from '@/components/layout/guest-layout';

const LoginPage = lazy(() => import('@/pages/auth/login'));
const RegisterPage = lazy(() => import('@/pages/auth/register'));
const ForgetPage = lazy(() => import('@/pages/auth/forget'));
const HomePage = lazy(() => import('@/pages/home'));
const DashboardPage = lazy(() => import('@/pages/dashboard'));
const PlansPage = lazy(() => import('@/pages/plans'));
const PlanCheckoutPage = lazy(() => import('@/pages/plans/checkout'));
const OrdersPage = lazy(() => import('@/pages/orders'));
const OrderDetailPage = lazy(() => import('@/pages/orders/detail'));
const ProfilePage = lazy(() => import('@/pages/profile'));
const InvitePage = lazy(() => import('@/pages/invite'));
const TicketsPage = lazy(() => import('@/pages/tickets'));
const TicketDetailPage = lazy(() => import('@/pages/tickets/detail'));
const KnowledgePage = lazy(() => import('@/pages/knowledge'));
const NodePage = lazy(() => import('@/pages/node'));
const TrafficPage = lazy(() => import('@/pages/traffic'));

function Pending() {
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
    <Suspense fallback={<Pending />}>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route element={<GuestLayout />}>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/register" element={<RegisterPage />} />
          <Route path="/forgetpassword" element={<ForgetPage />} />
        </Route>
        <Route element={<AppLayout />}>
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="/plan" element={<PlansPage />} />
          <Route path="/plan/:id" element={<PlanCheckoutPage />} />
          <Route path="/order" element={<OrdersPage />} />
          <Route path="/order/:tradeNo" element={<OrderDetailPage />} />
          <Route path="/profile" element={<ProfilePage />} />
          <Route path="/invite" element={<InvitePage />} />
          <Route path="/ticket" element={<TicketsPage />} />
          <Route path="/knowledge" element={<KnowledgePage />} />
          <Route path="/node" element={<NodePage />} />
          <Route path="/traffic" element={<TrafficPage />} />
        </Route>
        <Route path="/ticket/:id" element={<TicketDetailPage />} />
      </Routes>
    </Suspense>
  );
}
