import { createApiClient, user } from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { getAdminApiBaseUrl, getAdminSecurePath } from './runtime-config';
import { navigateToLogin } from './router-navigation';
import { clearStepUpGrant, getStepUpToken, resolveStepUpPrompt } from './step-up';

// Session-expiry teardown (the 401 session_expired handler below). Must NOT
// call the logout endpoint: the token is already dead server-side and the
// call would only 401 again into the same handler.
export function redirectToLogin(): void {
  // Close the re-auth prompt with the session: stepUp can never succeed
  // without a live session, so a dialog stranded over /login is a dead end.
  resolveStepUpPrompt();
  clearStepUpGrant();
  logout();
  navigateToLogin();
}

// Explicit sign-out only (the account menu). Admin sessions are ordinary user
// sessions to the backend, so the shared user endpoint revokes them too. Fires
// the best-effort revocation, then tears local auth down synchronously; the
// network call never blocks, delays, or fails the teardown. The raw auth_data
// is captured before teardown because the request interceptor reads the auth
// store on a microtask — after logout() has already cleared it; the endpoint
// puts the Bearer scheme on the wire.
export function signOut(): void {
  const authData = getAuthData();
  if (authData) {
    void user.logout(apiClient, authData).catch(() => {});
  }
  logout();
}

export const apiClient = createApiClient({
  baseURL: getAdminApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getLocale(),
  getStepUpToken,
  adminSecurePath: () => getAdminSecurePath(),
  nullFormValue: 'empty',
  // Fires only for the 401 session_expired problem (docs/api-dialect.md
  // §3.2). The 403 permission_denied / step_up_required verdicts for a live
  // admin session never reach this hook, so no message discrimination is
  // needed anymore — the code slug already did it in the client.
  onUnauthorized: () => {
    redirectToLogin();
  },
});
