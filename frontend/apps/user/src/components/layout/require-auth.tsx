import { useEffect, useState, type ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router';
import { getAuthData, subscribeAuth } from '@/lib/auth';

export function RequireAuth({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(() => getAuthData());
  const location = useLocation();
  const redirect = `${location.pathname}${location.search}`;
  const loginTarget = {
    pathname: '/login',
    search: `?redirect=${encodeURIComponent(redirect)}`,
  };

  useEffect(() => subscribeAuth(setToken), []);

  if (!token) {
    return <Navigate to={loginTarget} replace />;
  }
  return <>{children}</>;
}
