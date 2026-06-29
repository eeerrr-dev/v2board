import { type ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router';
import { useAuthData } from '@/lib/auth';

export function RequireAuth({ children }: { children: ReactNode }) {
  const token = useAuthData();
  const location = useLocation();
  const redirect = `${location.pathname}${location.search}`;
  const loginTarget = {
    pathname: '/login',
    search: `?redirect=${encodeURIComponent(redirect)}`,
  };

  if (!token) {
    return <Navigate to={loginTarget} replace />;
  }
  return <>{children}</>;
}
