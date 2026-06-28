import { createApiClient } from '@v2board/api-client';
import type { ApiError } from '@v2board/api-client';
import { legacyGetLocale } from '@v2board/i18n';
import { getAuthData, setAuthData } from './auth';
import { i18nGet } from './errors';
import { getLegacySettings } from './legacy-settings';
import { toast } from './toast';

let redirectingToLogin = false;

function redirectToLegacyLogin(): void {
  if (redirectingToLogin) return;
  redirectingToLogin = true;
  if (getAuthData() !== null) setAuthData(null);
  window.location.hash = '#/login';
  window.setTimeout(() => {
    redirectingToLogin = false;
  }, 0);
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
