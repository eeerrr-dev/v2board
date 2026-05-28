import { Suspense, lazy, useEffect } from 'react';
import { App as AntdApp, Spin } from 'antd';
import { Navigate, Route, Routes } from 'react-router-dom';
import { AdminLayout } from '@/components/layout/admin-layout';
import { RequireAuth } from '@/components/layout/require-auth';
import { bindMessageApi } from '@/lib/api';

const LoginPage = lazy(() => import('@/pages/login'));
const DashboardPage = lazy(() => import('@/pages/dashboard'));
const UsersPage = lazy(() => import('@/pages/users'));
const OrdersPage = lazy(() => import('@/pages/orders'));
const PlansPage = lazy(() => import('@/pages/plans'));
const ServersPage = lazy(() => import('@/pages/servers'));
const TicketsPage = lazy(() => import('@/pages/tickets'));
const PaymentsPage = lazy(() => import('@/pages/payments'));
const CouponsPage = lazy(() => import('@/pages/coupons'));
const KnowledgePage = lazy(() => import('@/pages/knowledge'));
const NoticesPage = lazy(() => import('@/pages/notices'));
const SystemPage = lazy(() => import('@/pages/system'));
const ConfigPage = lazy(() => import('@/pages/config'));
const StatsPage = lazy(() => import('@/pages/stats'));

function Fallback() {
  return (
    <div className="h-screen flex items-center justify-center">
      <Spin size="large" />
    </div>
  );
}

export default function App() {
  const { message } = AntdApp.useApp();
  useEffect(() => bindMessageApi(message), [message]);

  return (
    <Suspense fallback={<Fallback />}>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route
          element={
            <RequireAuth>
              <AdminLayout />
            </RequireAuth>
          }
        >
          <Route path="/" element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="/users" element={<UsersPage />} />
          <Route path="/orders" element={<OrdersPage />} />
          <Route path="/plans" element={<PlansPage />} />
          <Route path="/servers" element={<ServersPage />} />
          <Route path="/tickets" element={<TicketsPage />} />
          <Route path="/payments" element={<PaymentsPage />} />
          <Route path="/coupons" element={<CouponsPage />} />
          <Route path="/knowledge" element={<KnowledgePage />} />
          <Route path="/notices" element={<NoticesPage />} />
          <Route path="/system" element={<SystemPage />} />
          <Route path="/config" element={<ConfigPage />} />
          <Route path="/stats" element={<StatsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </Suspense>
  );
}
