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

// Subscribe React components to the auth token so a logout() (or token2Login)
// elsewhere re-renders guarded routes. Mirrors the dark-mode store hook.
export function useAuthData(): string | null {
  return useSyncExternalStore(subscribeAuth, getAuthData, getAuthData);
}
