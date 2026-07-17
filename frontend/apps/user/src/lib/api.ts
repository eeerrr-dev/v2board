import { createApiClient, user } from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { navigateToLogin } from './router-navigation';

// Session-expiry teardown (the 403 handler below). Must NOT call the logout
// endpoint: the token is already dead server-side and the call would only 403
// again into the same handler.
export function redirectToLogin(): void {
  logout();
  navigateToLogin();
}

// Explicit sign-out only (the account menu). Fires a best-effort server-side
// revocation of the current session, then tears local auth down synchronously;
// the network call never blocks, delays, or fails the teardown. The bearer is
// captured before teardown because the request interceptor reads the auth
// store on a microtask — after logout() has already cleared it.
export function signOut(): void {
  const authorization = getAuthData();
  if (authorization) {
    void user.logout(apiClient, { headers: { authorization } }).catch(() => {});
  }
  logout();
}

export const apiClient = createApiClient({
  baseURL: getApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getRequestLocale(),
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
