import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { apiClient } from './api';
import { getAuthData, registerSessionCacheClearer, setAuthData } from './auth';
import { registerRouterNavigation } from './router-navigation';
import { setAdminRuntimeConfig } from '@/test/runtime-config';

type Adapter = typeof apiClient.axios.defaults.adapter;
type AdapterFn = Extract<NonNullable<Adapter>, (...args: never[]) => unknown>;
type RouterNavigate = (to: '/login', options: { replace: true }) => Promise<void>;
const originalAdapter = apiClient.axios.defaults.adapter;

function adapterFor(status: number, data: unknown): AdapterFn {
  return async (config) => {
    const response = { config, data, headers: {}, status, statusText: `${status}` };
    if (config.validateStatus && !config.validateStatus(status)) {
      const error = new Error(`Request failed with status code ${status}`) as Error & {
        config: unknown;
        response: unknown;
        isAxiosError: boolean;
      };
      error.config = config;
      error.response = response;
      error.isAxiosError = true;
      throw error;
    }
    return response;
  };
}

function transportErrorAdapter(message: string): AdapterFn {
  return async (config) => {
    const error = new Error(message) as Error & {
      config: unknown;
      isAxiosError: boolean;
    };
    error.config = config;
    error.isAxiosError = true;
    throw error;
  };
}

describe('admin api legacy path resolution', () => {
  let routerNavigate: ReturnType<typeof vi.fn<RouterNavigate>>;

  beforeEach(() => {
    routerNavigate = vi.fn<RouterNavigate>().mockResolvedValue(undefined);
    registerRouterNavigation({ navigate: routerNavigate });
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
    apiClient.axios.defaults.adapter = originalAdapter;
    registerSessionCacheClearer(() => undefined);
    vi.restoreAllMocks();
    setAdminRuntimeConfig();
    window.g_lang = undefined;
    window.localStorage.clear();
  });

  it('prefixes admin endpoints with the bootstrapped secure_path', () => {
    setAdminRuntimeConfig({ secure_path: '/secret-admin' });

    expect(apiClient.resolveAdminPath('/plan/fetch')).toBe('/secret-admin/plan/fetch');
  });

  it('uses the canonical admin path when the runtime bootstrap is absent', () => {
    setAdminRuntimeConfig();

    expect(apiClient.resolveAdminPath('/plan/fetch')).toBe('/admin/plan/fetch');
  });

  it('keeps the admin API same-origin', async () => {
    vi.resetModules();
    setAdminRuntimeConfig({ secure_path: 'admin' });

    const { apiClient: hostedClient } = await import('./api');

    expect(hostedClient.axios.defaults.baseURL).toBe(
      `${new URL(window.location.href).origin}/api/v1`,
    );
  });

  it('sends the shared active locale in admin request headers', async () => {
    window.g_lang = 'ja-JP';
    const originalAdapter = apiClient.axios.defaults.adapter;
    let requestConfig: Parameters<AdapterFn>[0] | undefined;
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
      await apiClient.request({
        url: '/plan/fetch',
        method: 'GET',
        responseSchema: z.unknown(),
      });

      expect(requestConfig?.headers?.['Content-Language']).toBe('ja-JP');
    } finally {
      apiClient.axios.defaults.adapter = originalAdapter;
    }
  });

  it('repairs an unsupported persisted locale from the supported browser preference', async () => {
    vi.spyOn(window.navigator, 'languages', 'get').mockReturnValue(['ja-JP']);
    vi.spyOn(window.navigator, 'language', 'get').mockReturnValue('ja-JP');
    window.localStorage.setItem('umi_locale', 'fr-FR');
    window.g_lang = 'not-a-locale';
    let requestConfig: Parameters<AdapterFn>[0] | undefined;
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

    await apiClient.request({
      url: '/plan/fetch',
      method: 'GET',
      responseSchema: z.unknown(),
    });

    expect(requestConfig?.headers?.['Content-Language']).toBe('ja-JP');
  });

  it('clears the invalid credential, query cache and redirects to login on 403', async () => {
    const clear = vi.fn();
    registerSessionCacheClearer(clear);
    setAuthData('expired-admin');
    clear.mockClear();
    apiClient.axios.defaults.adapter = adapterFor(403, { message: 'auth required' });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403 });

    expect(getAuthData()).toBeNull();
    expect(clear).toHaveBeenCalledOnce();
    expect(routerNavigate).toHaveBeenCalledOnce();
    expect(routerNavigate).toHaveBeenCalledWith('/login', { replace: true });
  });

  it('returns typed backend and transport errors to their query or mutation owner', async () => {
    apiClient.axios.defaults.adapter = adapterFor(500, { message: 'server exploded' });

    await expect(
      apiClient.request({ url: '/plan/fetch', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 500, message: 'server exploded' });

    apiClient.axios.defaults.adapter = transportErrorAdapter('Network Error');
    await expect(
      apiClient.request({ url: '/plan/fetch', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 0, message: 'Network Error' });
  });
});
