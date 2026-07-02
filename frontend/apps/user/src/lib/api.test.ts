import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { apiClient } from './api';
import { getAuthData, registerSessionCacheClearer, setAuthData } from './auth';
import { toast } from './toast';

type Adapter = typeof apiClient.axios.defaults.adapter;
type AdapterFn = Extract<NonNullable<Adapter>, (...args: never[]) => unknown>;

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

  beforeEach(() => {
    vi.useFakeTimers();
    clearSessionSpy = vi.fn<() => void>();
    registerSessionCacheClearer(clearSessionSpy);
    setAuthData('token-403');
    window.location.hash = '#/dashboard';
  });

  afterEach(() => {
    // Run the restore timer so the module-level redirect guard re-arms for the
    // next test, then drop all session state again.
    vi.runAllTimers();
    vi.useRealTimers();
    apiClient.axios.defaults.adapter = originalAdapter;
    registerSessionCacheClearer(() => undefined);
    setAuthData(null);
    window.location.hash = '';
  });

  it('runs the legacy remove -> redirect -> restore dance on an HTTP 403', async () => {
    apiClient.axios.defaults.adapter = adapterFor(403, { message: 'auth required' });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET' }),
    ).rejects.toMatchObject({ status: 403 });

    // remove -> redirect: token dropped, session caches cleared, hash parked
    // on the login route.
    expect(getAuthData()).toBeNull();
    expect(window.location.hash).toBe('#/login');
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);

    // -> restore: the legacy 50ms timer puts the credential back (the oracle
    // run ends on #/login with authorization still set).
    vi.advanceTimersByTime(50);
    expect(getAuthData()).toBe('token-403');
  });

  it('runs the same teardown for a legacy envelope code 403 carried over HTTP 200', async () => {
    apiClient.axios.defaults.adapter = adapterFor(200, {
      code: 403,
      data: null,
      message: 'auth required',
    });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET' }),
    ).rejects.toMatchObject({ status: 403 });

    expect(getAuthData()).toBeNull();
    expect(window.location.hash).toBe('#/login');
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(50);
    expect(getAuthData()).toBe('token-403');
  });

  it('collapses concurrent 403s into a single redirect and teardown', async () => {
    apiClient.axios.defaults.adapter = adapterFor(403, { message: 'auth required' });

    const results = await Promise.allSettled([
      apiClient.request({ url: '/user/info', method: 'GET' }),
      apiClient.request({ url: '/user/getSubscribe', method: 'GET' }),
    ]);

    expect(results.map((result) => result.status)).toEqual(['rejected', 'rejected']);
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);
    expect(window.location.hash).toBe('#/login');

    vi.advanceTimersByTime(50);
    expect(getAuthData()).toBe('token-403');
  });
});

describe('user api global error toast', () => {
  const originalAdapter = apiClient.axios.defaults.adapter;

  afterEach(() => {
    apiClient.axios.defaults.adapter = originalAdapter;
    vi.restoreAllMocks();
  });

  it('stays silent on any transport failure, timeout or network alike', async () => {
    // The packaged user frontend used fetch, which rejected before its toast
    // code ran, so it surfaced nothing for ANY transport error (timeout or
    // network drop) — not just timeouts. No timeout-message sniffing.
    const errorToast = vi.spyOn(toast, 'error').mockReturnValue('toast-id');

    apiClient.axios.defaults.adapter = transportErrorAdapter(
      'timeout of 8000ms exceeded',
      'ECONNABORTED',
    );
    await expect(
      apiClient.request({ url: '/user/info', method: 'GET' }),
    ).rejects.toMatchObject({ status: 0, message: 'timeout of 8000ms exceeded' });

    apiClient.axios.defaults.adapter = transportErrorAdapter('Network Error');
    await expect(
      apiClient.request({ url: '/user/info', method: 'GET' }),
    ).rejects.toMatchObject({ status: 0, message: 'Network Error' });

    expect(errorToast).not.toHaveBeenCalled();
  });

  it('toasts the localized 请求失败 with the backend message for other non-200 responses', async () => {
    const errorToast = vi.spyOn(toast, 'error').mockReturnValue('toast-id');

    apiClient.axios.defaults.adapter = adapterFor(500, { message: 'server exploded' });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET' }),
    ).rejects.toMatchObject({ status: 500, message: 'server exploded' });

    expect(errorToast).toHaveBeenCalledTimes(1);
    // zh-CN is the default error-dictionary locale in tests; '请求失败' maps to itself.
    expect(errorToast).toHaveBeenCalledWith('请求失败', { description: 'server exploded' });
  });
});
