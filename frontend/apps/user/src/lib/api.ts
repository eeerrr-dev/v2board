import { createApiClient } from '@v2board/api-client';
import type { ApiError } from '@v2board/api-client';
import { legacyGetLocale } from '@v2board/i18n';
import { getAuthData } from './auth';
import { i18nGet } from './errors';
import { getLegacySettings } from './legacy-settings';
import { toast } from './toast';

let redirectingToLogin = false;
const LEGACY_AUTH_STORAGE_KEY = 'authorization';

function redirectToLegacyLogin(): void {
  if (redirectingToLogin) return;
  redirectingToLogin = true;
  const authData = getAuthData();
  if (authData !== null) {
    window.localStorage.removeItem(LEGACY_AUTH_STORAGE_KEY);
  }
  window.location.hash = '#/login';
  if (authData !== null) {
    window.setTimeout(() => {
      window.localStorage.setItem(LEGACY_AUTH_STORAGE_KEY, authData);
    }, 50);
  }
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
