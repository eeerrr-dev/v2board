import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { apiClient } from './api';
import { getAuthData, registerSessionCacheClearer, setAuthData } from './auth';

const apiSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'api.ts'), 'utf8');

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

describe('user api unauthorized handling', () => {
  const originalAdapter = apiClient.axios.defaults.adapter;
  let clearSessionSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    clearSessionSpy = vi.fn();
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

  it('never touches legacy storage keys or full-page redirects (anti-legacy pins)', () => {
    expect(apiSource).not.toContain('logout();');
    expect(apiSource).not.toContain('LEGACY_AUTH_STORAGE_KEY');
    expect(apiSource).not.toContain('window.localStorage.setItem');
    expect(apiSource).not.toContain('window.location.pathname}#/login');
    expect(apiSource).not.toContain('window.location.href = `${window.location.origin}/#/login`;');
    expect(apiSource).not.toContain("window.location.href = '/';");
    expect(apiSource).not.toContain("window.location.replace('/#/login');");
  });
});

describe('user api global error toast', () => {
  it('stays silent on any transport failure and toasts other non-200 responses', () => {
    // The packaged user frontend used fetch, which rejected before its toast code ran, so it
    // surfaced nothing for transport errors (timeout or network) — not just timeouts.
    expect(apiSource).toContain('if (error.status === 0) return;');
    expect(apiSource).toContain(
      "toast.error(i18nGet('请求失败'), { description: error.message });",
    );
    expect(apiSource).not.toContain('isLegacyTimeoutError');
    expect(apiSource).not.toContain('/timeout/i.test');
  });
});
