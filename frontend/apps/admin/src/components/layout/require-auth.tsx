import { useEffect, useState, type ReactNode } from 'react';
import { Navigate } from 'react-router-dom';
import { getAuthData, subscribeAuth } from '@/lib/auth';

export function RequireAuth({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(() => getAuthData());
  useEffect(() => subscribeAuth(setToken), []);
  if (!token) return <Navigate to="/login" replace />;
  return <>{children}</>;
}
