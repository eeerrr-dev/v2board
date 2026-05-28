import { createApiClient } from '@v2board/api-client';
import type { ApiError } from '@v2board/api-client';
import { getAuthData, logout } from './auth';
import { i18nGet } from './errors';
import { getLegacyCookie } from './legacy-cookie';
import { getLegacySettings } from './legacy-settings';
import { toast } from './legacy-toast';

export const apiClient = createApiClient({
  baseURL: getApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getRequestLocale(),
  onUnauthorized: () => {
    logout();
    window.location.href = '/';
  },
  onError: (error: ApiError) => {
    toast.error(i18nGet('请求失败'), { description: i18nGet(error.message) });
  },
});

function getApiBaseUrl(): string {
  const host = getLegacySettings().host;
  const origin = new URL(window.location.href).origin;
  return `${host || origin}/api/v1`;
}

function getRequestLocale(): string {
  return (
    getLegacyCookie('i18n') ||
    window.localStorage.getItem('umi_locale') ||
    window.navigator.language
  );
}
