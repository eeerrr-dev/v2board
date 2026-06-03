import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { InternalAxiosRequestConfig } from 'axios';
import { apiClient } from './api';

const apiSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'api.ts'), 'utf8');

describe('admin api legacy path resolution', () => {
  afterEach(() => {
    window.settings = undefined;
    window.g_lang = undefined;
    localStorage.clear();
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

  it('keeps the old 403 behavior: clear auth and reload the current admin entry', () => {
    expect(apiSource).toContain('logout();');
    expect(apiSource).toContain('window.location.href = `${window.location.origin}${window.location.pathname}`;');
    expect(apiSource).not.toContain("window.location.replace('/login')");
  });
});
