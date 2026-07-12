import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { apiClient } from './api';
import { getAuthData, registerSessionCacheClearer, setAuthData } from './auth';
import { registerRouterNavigation } from './router-navigation';

type Adapter = typeof apiClient.axios.defaults.adapter;
type AdapterFn = Extract<NonNullable<Adapter>, (...args: never[]) => unknown>;
type RouterNavigate = (to: '/login', options: { replace: true }) => Promise<void>;

// Stands in for the network like axios' own adapters do (settle semantics):
// resolve on validateStatus, reject with the response attached otherwise.
// Typed off the live client on purpose — axios is a dependency of the
// api-client package, not of the user app, so this test must not import it.
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

// A transport-level failure (timeout or network drop): the adapter rejects with
// no response attached, exactly like axios' own adapters do.
function transportErrorAdapter(message: string, code?: string): AdapterFn {
  return async (config) => {
    const error = new Error(message) as Error & {
      config: unknown;
      code?: string;
      isAxiosError: boolean;
    };
    error.config = config;
    if (code) error.code = code;
    error.isAxiosError = true;
    throw error;
  };
}

describe('user api unauthorized handling', () => {
  const originalAdapter = apiClient.axios.defaults.adapter;
  let clearSessionSpy: ReturnType<typeof vi.fn<() => void>>;
  let routerNavigate: ReturnType<typeof vi.fn<RouterNavigate>>;

  beforeEach(() => {
    routerNavigate = vi.fn<RouterNavigate>().mockResolvedValue(undefined);
    registerRouterNavigation({ navigate: routerNavigate });
    clearSessionSpy = vi.fn<() => void>();
    registerSessionCacheClearer(clearSessionSpy);
    setAuthData('token-403');
    clearSessionSpy.mockClear();
  });

  afterEach(() => {
    apiClient.axios.defaults.adapter = originalAdapter;
    registerSessionCacheClearer(() => undefined);
    setAuthData(null);
  });

  it('permanently clears the credential and redirects on an HTTP 403', async () => {
    apiClient.axios.defaults.adapter = adapterFor(403, { message: 'auth required' });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403 });

    expect(getAuthData()).toBeNull();
    expect(routerNavigate).toHaveBeenCalledOnce();
    expect(routerNavigate).toHaveBeenCalledWith('/login', { replace: true });
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);
  });

  it('runs the same teardown for a legacy envelope code 403 carried over HTTP 200', async () => {
    apiClient.axios.defaults.adapter = adapterFor(200, {
      code: 403,
      data: null,
      message: 'auth required',
    });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403 });

    expect(getAuthData()).toBeNull();
    expect(routerNavigate).toHaveBeenCalledOnce();
    expect(routerNavigate).toHaveBeenCalledWith('/login', { replace: true });
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);

    expect(getAuthData()).toBeNull();
  });

  it('keeps concurrent 403 teardown idempotent', async () => {
    apiClient.axios.defaults.adapter = adapterFor(403, { message: 'auth required' });

    const results = await Promise.allSettled([
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
      apiClient.request({
        url: '/user/getSubscribe',
        method: 'GET',
        responseSchema: z.unknown(),
      }),
    ]);

    expect(results.map((result) => result.status)).toEqual(['rejected', 'rejected']);
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);
    expect(routerNavigate).toHaveBeenCalledTimes(2);
    expect(routerNavigate).toHaveBeenCalledWith('/login', { replace: true });

    expect(getAuthData()).toBeNull();
  });
});

describe('user api typed errors', () => {
  const originalAdapter = apiClient.axios.defaults.adapter;

  afterEach(() => {
    apiClient.axios.defaults.adapter = originalAdapter;
    vi.restoreAllMocks();
  });

  it('throws transport failures without coupling the client to toast presentation', async () => {
    apiClient.axios.defaults.adapter = transportErrorAdapter(
      'timeout of 8000ms exceeded',
      'ECONNABORTED',
    );
    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 0, message: 'timeout of 8000ms exceeded' });

    apiClient.axios.defaults.adapter = transportErrorAdapter('Network Error');
    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 0, message: 'Network Error' });
  });

  it('preserves the backend message for the query or mutation owner to present', async () => {
    apiClient.axios.defaults.adapter = adapterFor(500, { message: 'server exploded' });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 500, message: 'server exploded' });
  });
});
