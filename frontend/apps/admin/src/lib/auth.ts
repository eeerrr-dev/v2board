import { useSyncExternalStore } from 'react';

const AUTH_KEY = 'authorization';

export interface AdminLoginLoaderData {
  redirectTarget: string;
}

type Listener = (value: string | null) => void;
const listeners = new Set<Listener>();
let authSyncInstalled = false;
let sessionCacheClearer: (() => void) | undefined;

export function getAuthData(): string | null {
  return localStorage.getItem(AUTH_KEY);
}

export function setAuthData(value: string | null): void {
  const previous = getAuthData();
  if (previous === value) return;

  if (value === null) localStorage.removeItem(AUTH_KEY);
  else localStorage.setItem(AUTH_KEY, value);
  sessionCacheClearer?.();
  for (const l of listeners) l(value);
}

export function subscribeAuth(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setupAuthSync(): void {
  if (authSyncInstalled) return;
  authSyncInstalled = true;
  window.addEventListener('storage', (event) => {
    if (event.key === AUTH_KEY) {
      sessionCacheClearer?.();
      for (const l of listeners) l(event.newValue);
    }
  });
}

export function registerSessionCacheClearer(clearer: () => void): void {
  sessionCacheClearer = clearer;
}

export function logout(): void {
  setAuthData(null);
}

export function buildLoginRedirect(current: string): string {
  return `/login?redirect=${encodeURIComponent(current)}`;
}

export function useAuthData(): string | null {
  return useSyncExternalStore(subscribeAuth, getAuthData, getAuthData);
}
