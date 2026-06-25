import { createApiClient } from '@v2board/api-client';
import type { App } from 'antd';
import { getAuthData, setAuthData } from './auth';
import { i18nGet } from './errors';
import { getAdminApiBaseUrl, getAdminSecurePath } from './legacy-settings';

let notificationApi: ReturnType<typeof App.useApp>['notification'] | null = null;
export function bindNotificationApi(api: ReturnType<typeof App.useApp>['notification']): void {
  notificationApi = api;
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
    // Transport-level failures (timeouts/network drops) have no backend response;
    // the legacy admin never surfaced these as a global toast.
    if (error.status === 0) return;
    // Faithful to the packaged admin: every non-200 backend response (except the
    // 403 redirect handled by onUnauthorized) raised a single global
    // notification with the first validation error or the response message.
    notificationApi?.error({
      message: i18nGet('请求失败'),
      description: i18nGet(error.message),
      duration: 1.5,
    });
  },
});
