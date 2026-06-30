import { type ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router';
import { buildLoginRedirect, useAuthData } from '@/lib/auth';

export function RequireAuth({ children }: { children: ReactNode }) {
  const token = useAuthData();
  const location = useLocation();

  if (!token) {
    return <Navigate to={buildLoginRedirect(`${location.pathname}${location.search}`)} replace />;
  }
  return <>{children}</>;
}
