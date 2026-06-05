import { createApiClient } from '@v2board/api-client';
import type { App } from 'antd';
import { getAuthData, logout } from './auth';
import { i18nGet } from './errors';
import { getAdminApiBaseUrl, getAdminSecurePath } from './legacy-settings';

let messageApi: ReturnType<typeof App.useApp>['message'] | null = null;
export function bindMessageApi(api: ReturnType<typeof App.useApp>['message']): void {
  messageApi = api;
}

let redirectingToLogin = false;

function redirectToLegacyLogin(): void {
  if (redirectingToLogin) return;
  redirectingToLogin = true;
  window.location.href = `${window.location.origin}/#/login`;
}

export const apiClient = createApiClient({
  baseURL: getAdminApiBaseUrl(),
  getAuthData: () => getAuthData(),
  adminSecurePath: () => getAdminSecurePath(),
  nullFormValue: 'empty',
  onUnauthorized: () => {
    logout();
    redirectToLegacyLogin();
  },
  onError: (error) => {
    if (error.status === 0 || error.status >= 500) {
      messageApi?.error(i18nGet(error.message));
    }
  },
});
