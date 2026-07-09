import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { InternalAxiosRequestConfig } from 'axios';
import { apiClient } from './api';

const apiSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'api.ts'), 'utf8');

describe('admin api legacy path resolution', () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    const storage = {
      clear: () => store.clear(),
      getItem: (key: string) => store.get(key) ?? null,
      removeItem: (key: string) => {
        store.delete(key);
      },
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
    };
    Object.defineProperty(window, 'localStorage', {
      configurable: true,
      value: storage,
    });
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: storage,
    });
  });

  afterEach(() => {
    window.settings = undefined;
    window.g_lang = undefined;
    window.localStorage.clear();
  });

  it('prefixes admin endpoints with window.settings.secure_path', () => {
    window.settings = { secure_path: '/secret-admin' };

    expect(apiClient.resolveAdminPath('/plan/fetch')).toBe('/secret-admin/plan/fetch');
  });

  it('falls back to the original endpoint path when secure_path is not present', () => {
    window.settings = {};

    expect(apiClient.resolveAdminPath('/plan/fetch')).toBe('/plan/fetch');
  });

  it('uses window.settings.host for the legacy admin service host', async () => {
    vi.resetModules();
    window.settings = { host: 'https://api.example.com', secure_path: 'admin' };

    const { apiClient: hostedClient } = await import('./api');

    expect(hostedClient.axios.defaults.baseURL).toBe('https://api.example.com/api/v1');
  });

  it('does not send the user-bundle locale header on admin requests', async () => {
    window.g_lang = 'ja-JP';
    const originalAdapter = apiClient.axios.defaults.adapter;
    let requestConfig: InternalAxiosRequestConfig | undefined;
    apiClient.axios.defaults.adapter = async (config) => {
      requestConfig = config;
      return {
        data: { data: [] },
        status: 200,
        statusText: 'OK',
        headers: {},
        config,
      };
    };

    try {
      await apiClient.request({ url: '/plan/fetch', method: 'GET' });

      expect(requestConfig?.headers?.['Content-Language']).toBeUndefined();
    } finally {
      apiClient.axios.defaults.adapter = originalAdapter;
    }
  });

  it('keeps auth and redirects once to the hash login route on 403', () => {
    expect(apiSource).not.toContain('logout();');
    expect(apiSource).toContain('let redirectingToLogin = false;');
    expect(apiSource).toContain('if (redirectingToLogin) return;');
    expect(apiSource).toContain('redirectingToLogin = true;');
    expect(apiSource).toContain('setAuthData(null);');
    expect(apiSource).toContain('replaceLegacyLoginHash();');
    expect(apiSource).toContain("`${window.location.pathname}${window.location.search}#/login`");
    expect(apiSource).toContain("new HashChangeEvent('hashchange'");
    expect(apiSource).toContain("new PopStateEvent('popstate')");
    expect(apiSource).toContain('restoreAuthAfterLoginRendered(authData);');
    expect(apiSource).toContain("document.querySelector('.v2board-auth-box')");
    expect(apiSource).toContain('setAuthData(authData);');
    expect(apiSource).toContain('window.setTimeout(restore, 0);');
    expect(apiSource).not.toContain('attemptsLeft <= 0');
    expect(apiSource).not.toContain('data-v2board-admin-redirect');
    expect(apiSource).not.toContain('window.location.pathname}#/login');
    expect(apiSource).not.toContain('window.location.href = `${window.location.origin}/#/login`;');
    expect(apiSource).not.toContain(
      'window.location.href = window.location.origin + window.location.pathname;',
    );
    expect(apiSource).not.toContain('window.location.replace');
  });

  it('raises a global notification for backend errors but stays silent on transport timeouts', () => {
    // Faithful to the packaged admin: every non-200 backend response raises a
    // single global notification ("请求失败" + first validation error / message),
    // while transport-level failures (status 0) show nothing. The antd static
    // notification API was replaced by the shadcn island toaster.
    expect(apiSource).toContain('if (error.status === 0) return;');
    expect(apiSource).toContain('toast.error(i18nGet(\'请求失败\'), {');
    expect(apiSource).toContain('description: i18nGet(error.message),');
    expect(apiSource).toContain('duration: 1500,');
    expect(apiSource).not.toContain('notificationApi');
    expect(apiSource).not.toContain("from 'antd'");
    expect(apiSource).not.toContain('if (error.status >= 500) {');
  });
});
