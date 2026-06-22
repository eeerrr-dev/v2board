import { createApiClient } from '@v2board/api-client';
import type { App } from 'antd';
import { getAuthData, setAuthData } from './auth';
import { i18nGet } from './errors';
import { getAdminApiBaseUrl, getAdminSecurePath } from './legacy-settings';

let messageApi: ReturnType<typeof App.useApp>['message'] | null = null;
export function bindMessageApi(api: ReturnType<typeof App.useApp>['message']): void {
  messageApi = api;
}

let redirectingToLogin = false;

export function redirectToLegacyLogin(): void {
  if (redirectingToLogin) return;
  redirectingToLogin = true;
  const authData = getAuthData();
  if (authData !== null) {
    setAuthData(null);
  }
  replaceLegacyLoginHash();
  if (authData !== null) {
    restoreAuthAfterLoginRendered(authData);
  }
}

function replaceLegacyLoginHash(): void {
  const oldUrl = window.location.href;
  window.history.replaceState(
    window.history.state,
    '',
    `${window.location.pathname}${window.location.search}#/login`,
  );
  window.dispatchEvent(
    new HashChangeEvent('hashchange', { oldURL: oldUrl, newURL: window.location.href }),
  );
  window.dispatchEvent(new PopStateEvent('popstate'));
}

function restoreAuthAfterLoginRendered(authData: string): void {
  const restore = (): void => {
    if (document.querySelector('.v2board-auth-box')) {
      setAuthData(authData);
      return;
    }
    window.setTimeout(restore, 100);
  };
  window.setTimeout(restore, 0);
}

export const apiClient = createApiClient({
  baseURL: getAdminApiBaseUrl(),
  getAuthData: () => getAuthData(),
  adminSecurePath: () => getAdminSecurePath(),
  nullFormValue: 'empty',
  onUnauthorized: () => {
    redirectToLegacyLogin();
  },
  onError: (error) => {
    if (error.status === 0 || error.status >= 500) {
      messageApi?.error(i18nGet(error.message));
    }
  },
});
