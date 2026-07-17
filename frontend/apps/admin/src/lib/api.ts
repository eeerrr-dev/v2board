import {
  createApiClient,
  PERMISSION_DENIED_MESSAGE,
  STEP_UP_REQUIRED_MESSAGE,
  user,
} from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { getAdminApiBaseUrl, getAdminSecurePath } from './runtime-config';
import { navigateToLogin } from './router-navigation';
import { clearStepUpGrant, getStepUpToken, resolveStepUpPrompt } from './step-up';

// Session-expiry teardown (the 403 handler below). Must NOT call the logout
// endpoint: the token is already dead server-side and the call would only 403
// again into the same handler.
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
// network call never blocks, delays, or fails the teardown. The bearer is
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
  baseURL: getAdminApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getLocale(),
  getStepUpToken,
  adminSecurePath: () => getAdminSecurePath(),
  nullFormValue: 'empty',
  onUnauthorized: (error) => {
    // Discriminate 403s by exclusion: these two messages are authorization
    // verdicts for a live admin session (the role gate and the privileged
    // step-up gate in Rust auth.rs) and must not end it. Any other 403 —
    // including an expired session — keeps the fail-safe teardown.
    if (error.message === PERMISSION_DENIED_MESSAGE || error.message === STEP_UP_REQUIRED_MESSAGE) {
      return;
    }
    redirectToLogin();
  },
});
