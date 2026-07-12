import { createApiClient } from '@v2board/api-client';
import { getLocale } from '@v2board/i18n';
import { getAuthData, logout } from './auth';
import { getAdminApiBaseUrl, getAdminSecurePath } from './runtime-config';
import { navigateToLogin } from './router-navigation';

export function redirectToLogin(): void {
  logout();
  navigateToLogin();
}

export const apiClient = createApiClient({
  baseURL: getAdminApiBaseUrl(),
  getAuthData: () => getAuthData(),
  getLocale: () => getLocale(),
  adminSecurePath: () => getAdminSecurePath(),
  nullFormValue: 'empty',
  onUnauthorized: () => {
    redirectToLogin();
  },
});
