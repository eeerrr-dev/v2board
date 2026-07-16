import {
  createApiClient,
  PERMISSION_DENIED_MESSAGE,
  STEP_UP_REQUIRED_MESSAGE,
} from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { getAdminApiBaseUrl, getAdminSecurePath } from './runtime-config';
import { navigateToLogin } from './router-navigation';
import { clearStepUpGrant, getStepUpToken, resolveStepUpPrompt } from './step-up';

export function redirectToLogin(): void {
  // Close the re-auth prompt with the session: stepUp can never succeed
  // without a live session, so a dialog stranded over /login is a dead end.
  resolveStepUpPrompt();
  clearStepUpGrant();
  logout();
  navigateToLogin();
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
