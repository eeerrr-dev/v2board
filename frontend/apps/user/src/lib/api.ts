import { createApiClient } from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { navigateToLogin } from './router-navigation';

export function redirectToLogin(): void {
  logout();
  navigateToLogin();
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
