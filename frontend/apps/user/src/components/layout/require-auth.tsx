import { useEffect, useState, type ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { getAuthData, subscribeAuth } from '@/lib/auth';

export function RequireAuth({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(() => getAuthData());
  const location = useLocation();

  useEffect(() => subscribeAuth(setToken), []);

  if (!token) {
    return <Navigate to="/login" replace state={{ redirect: location.pathname + location.search }} />;
  }
  return <>{children}</>;
}
