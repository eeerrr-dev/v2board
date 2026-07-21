import { useSyncExternalStore } from 'react';
import { clearStepUpGrant } from './step-up';

// Admin-only pinned session key, independent of the user app's shared
// `authorization` key (see AGENTS.md Frontend Contract Direction and
// ADR-0003); also consumed by the route-normalization guard via
// ADMIN_ROUTE_GUARD_OPTIONS.
export const AUTH_KEY = 'v2board.admin_auth_data';

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
  // The step-up grant is bound server-side to (user_id, session_id), so any
  // identity change invalidates it — and Rust rejects a stale header outright
  // instead of falling back to the fresh login's recent-password window.
  clearStepUpGrant();
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
      clearStepUpGrant();
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
