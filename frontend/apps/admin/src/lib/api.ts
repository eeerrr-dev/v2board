import { createApiClient } from '@v2board/api-client';
import type { App } from 'antd';
import { getAuthData, getSecurePath, logout } from './auth';
import { i18nGet } from './errors';

let messageApi: ReturnType<typeof App.useApp>['message'] | null = null;
export function bindMessageApi(api: ReturnType<typeof App.useApp>['message']): void {
  messageApi = api;
}

export const apiClient = createApiClient({
  baseURL: '/api/v1',
  getAuthData: () => getAuthData(),
  adminSecurePath: () => getSecurePath(),
  onUnauthorized: () => {
    logout();
    if (!window.location.pathname.endsWith('/login')) {
      window.location.replace('/login');
    }
  },
  onError: (error) => {
    if (error.status === 0 || error.status >= 500) {
      messageApi?.error(i18nGet(error.message));
    }
  },
});
