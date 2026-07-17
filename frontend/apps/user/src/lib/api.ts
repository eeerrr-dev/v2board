import { createApiClient, user } from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { navigateToLogin } from './router-navigation';

// Session-expiry teardown (the 401 session_expired handler below). Must NOT
// call the logout endpoint: the token is already dead server-side and the
// call would only 401 again into the same handler.
export function redirectToLogin(): void {
  logout();
  navigateToLogin();
}

// Explicit sign-out only (the account menu). Fires a best-effort server-side
// revocation of the current session, then tears local auth down synchronously;
// the network call never blocks, delays, or fails the teardown. The raw
// auth_data is captured before teardown because the request interceptor reads
// the auth store on a microtask — after logout() has already cleared it; the
// endpoint puts the Bearer scheme on the wire.
export function signOut(): void {
  const authData = getAuthData();
  if (authData) {
    void user.logout(apiClient, authData).catch(() => {});
  }
  logout();
}

export const apiClient = createApiClient({
  baseURL: getApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getRequestLocale(),
  // Fires only for the 401 session_expired problem (docs/api-dialect.md §3.2);
  // 403 authorization verdicts never reach this hook.
  onUnauthorized: () => {
    redirectToLogin();
  },
});

function getApiBaseUrl(): string {
  const origin = new URL(window.location.href).origin;
  return `${origin}/api/v1`;
}

export function getRequestLocale(): string {
  return getLocale();
}
