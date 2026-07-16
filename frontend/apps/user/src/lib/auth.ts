import { useSyncExternalStore } from 'react';

// Tier-1 pinned session key; also consumed by the pre-router hash gate via
// USER_HASH_ROUTE_OPTIONS.
export const AUTH_STORAGE_KEY = 'authorization';
const listeners = new Set<(value: string | null) => void>();
let authSyncInstalled = false;

export function getAuthData(): string | null {
  return localStorage.getItem(AUTH_STORAGE_KEY);
}

function notifyAuthChange(value = getAuthData()): void {
  listeners.forEach((listener) => listener(value));
}

export function setAuthData(value: string | null): void {
  const previous = getAuthData();
  if (previous === value) return;

  if (value === null) {
    localStorage.removeItem(AUTH_STORAGE_KEY);
  } else {
    localStorage.setItem(AUTH_STORAGE_KEY, value);
  }
  clearSessionCaches();
  notifyAuthChange(value);
}

export function subscribeAuth(listener: (value: string | null) => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

// The QueryClient (created in main.tsx) holds per-session server state — user
// info, orders, invite stats, and the subscribe record whose subscribe_url is
// effectively a credential. Auth teardown must drop all of it so the next
// session on this tab cannot read the previous account's cached data. main.tsx
// registers the clearer at boot; lib code never imports the app entry.
let sessionCacheClearer: (() => void) | undefined;

export function registerSessionCacheClearer(clearer: () => void): void {
  sessionCacheClearer = clearer;
}

export function clearSessionCaches(): void {
  sessionCacheClearer?.();
}

export function setupAuthSync(): void {
  if (authSyncInstalled) return;
  authSyncInstalled = true;
  window.addEventListener('storage', (event) => {
    if (event.key !== AUTH_STORAGE_KEY) return;
    clearSessionCaches();
    notifyAuthChange(event.newValue);
  });
}

export function logout(): void {
  setAuthData(null);
}

// Single source of truth for the auth gate's login redirect. The login page
// reads the `redirect` query param to bounce the user back after sign-in, so
// both gate layers — the entry loader (App.tsx) and the live-session guard
// (require-auth.tsx) — must encode the return path identically. `current` is the
// `pathname + search` the user should return to.
export function buildLoginRedirect(current: string): string {
  return `/login?redirect=${encodeURIComponent(current)}`;
}

/**
 * Resolve the post-login destination as an internal route. This is shared by
 * the /login loader (existing sessions) and the login controller (new/verify
 * sessions), so protocol-relative and browser-normalized backslash bypasses
 * cannot drift between the two entry paths.
 *
 * The bare-path repair (`order` -> `/order`) is contract-driven and must stay:
 * the backend emits bare route names into `redirect` (login_redirect_url
 * defaults to `dashboard` in sessions.rs, and getQuickLoginUrl consumers pass
 * the same shape). The admin app's stricter variant (App.tsx: reject bare
 * paths) is intentionally different because nothing external deep-links into
 * admin; do not unify the two.
 */
export function normalizeLoginRedirectTarget(target: string | null): string {
  if (!target) return '/dashboard';
  const normalized = target
    .replace(/[\t\n\r]/g, '')
    .trim()
    .replace(/\\/g, '/');
  if (normalized.startsWith('//')) return '/dashboard';
  return normalized.startsWith('/') ? normalized : `/${normalized}`;
}

// Subscribe React components to the auth token so a logout() (or token2Login)
// elsewhere re-renders guarded routes. Mirrors the dark-mode store hook.
export function useAuthData(): string | null {
  return useSyncExternalStore(subscribeAuth, getAuthData, getAuthData);
}
