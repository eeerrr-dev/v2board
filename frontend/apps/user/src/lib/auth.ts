import { useSyncExternalStore } from 'react';

const AUTH_STORAGE_KEY = 'authorization';
const listeners = new Set<(value: string | null) => void>();

export function getAuthData(): string | null {
  return localStorage.getItem(AUTH_STORAGE_KEY);
}

function notifyAuthChange(value = getAuthData()): void {
  listeners.forEach((listener) => listener(value));
}

export function setAuthData(value: string | null): void {
  if (value === null) {
    localStorage.removeItem(AUTH_STORAGE_KEY);
  } else {
    localStorage.setItem(AUTH_STORAGE_KEY, value);
  }
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

export function logout(): void {
  setAuthData(null);
  clearSessionCaches();
}

// Single source of truth for the auth gate's login redirect. The login page
// reads the `redirect` query param to bounce the user back after sign-in, so
// both gate layers — the entry loader (App.tsx) and the live-session guard
// (require-auth.tsx) — must encode the return path identically. `current` is the
// `pathname + search` the user should return to.
export function buildLoginRedirect(current: string): string {
  return `/login?redirect=${encodeURIComponent(current)}`;
}

// Subscribe React components to the auth token so a logout() (or token2Login)
// elsewhere re-renders guarded routes. Mirrors the dark-mode store hook.
export function useAuthData(): string | null {
  return useSyncExternalStore(subscribeAuth, getAuthData, getAuthData);
}
