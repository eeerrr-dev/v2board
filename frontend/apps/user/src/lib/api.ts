import { createApiClient } from '@v2board/api-client';
import type { ApiError } from '@v2board/api-client';
import { legacyGetLocale } from '@v2board/i18n';
import { clearSessionCaches, getAuthData, setAuthData } from './auth';
import { i18nGet } from './errors';
import { getLegacySettings } from './legacy-settings';
import { toast } from './toast';

let redirectingToLogin = false;

// Legacy 403 ("session expired") teardown, restored byte-for-byte in outcome:
// drop the token so the auth gates bounce to the login screen, then put it
// back 50ms later exactly like the packaged frontend did — the oracle run ends
// parked on #/login with the credential still in storage (pinned by the
// user-session-expired-redirect interaction scenario). Cached server state
// from the torn-down session is cleared so the next account on this tab cannot
// read it. The guard collapses concurrent 403s into a single teardown and
// re-arms once the restore timer has run.
function redirectToLegacyLogin(): void {
  if (redirectingToLogin) return;
  redirectingToLogin = true;
  const authData = getAuthData();
  if (authData !== null) setAuthData(null);
  clearSessionCaches();
  window.location.hash = '#/login';
  window.setTimeout(() => {
    if (authData !== null) setAuthData(authData);
    redirectingToLogin = false;
  }, 50);
}

export const apiClient = createApiClient({
  baseURL: getApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getRequestLocale(),
  onUnauthorized: () => {
    redirectToLegacyLogin();
  },
  onError: (error: ApiError) => {
    // Transport-level failures (timeouts/network drops) have no backend response. The packaged
    // user frontend used fetch, which rejected before its toast code ran, so it surfaced nothing
    // for any transport error — only responses with a status raised the global "请求失败" toast.
    if (error.status === 0) return;
    toast.error(i18nGet('请求失败'), { description: error.message });
  },
});

function getApiBaseUrl(): string {
  const host = getLegacySettings().host;
  const origin = new URL(window.location.href).origin;
  return `${host || origin}/api/v1`;
}

export function getRequestLocale(): string {
  return legacyGetLocale();
}
