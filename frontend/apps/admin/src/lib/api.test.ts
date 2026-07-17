import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { ApiError } from '@v2board/api-client';
import { apiClient, signOut } from './api';
import {
  AUTH_KEY,
  getAuthData,
  registerSessionCacheClearer,
  setAuthData,
  setupAuthSync,
} from './auth';
import { registerRouterNavigation } from './router-navigation';
import {
  clearStepUpGrant,
  getStepUpToken,
  isStepUpPromptRequested,
  maybePromptStepUp,
  resolveStepUpPrompt,
  setStepUpGrant,
} from './step-up';
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
    window.localStorage.clear();
    resolveStepUpPrompt();
    clearStepUpGrant();
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
    window.localStorage.setItem('v2board_locale', 'ja-JP');
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

      // §4.3: Accept-Language is the locale signal; Content-Language rides
      // along transitionally until the legacy localization middleware retires.
      expect(requestConfig?.headers?.['Accept-Language']).toBe('ja-JP');
      expect(requestConfig?.headers?.['Content-Language']).toBe('ja-JP');
    } finally {
      apiClient.axios.defaults.adapter = originalAdapter;
    }
  });

  it('repairs an unsupported persisted locale from the supported browser preference', async () => {
    vi.spyOn(window.navigator, 'languages', 'get').mockReturnValue(['ja-JP']);
    vi.spyOn(window.navigator, 'language', 'get').mockReturnValue('ja-JP');
    window.localStorage.setItem('v2board_locale', 'fr-FR');
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

    expect(requestConfig?.headers?.['Accept-Language']).toBe('ja-JP');
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

  it('keeps the session on the live-session 403 verdicts (role gate, step-up gate)', async () => {
    const clear = vi.fn();
    registerSessionCacheClearer(clear);
    setAuthData('live-admin');
    clear.mockClear();

    for (const message of ['Permission denied', 'Recent password verification is required']) {
      apiClient.axios.defaults.adapter = adapterFor(403, { message });
      await expect(
        apiClient.request({ url: '/user/update', method: 'POST', responseSchema: z.unknown() }),
      ).rejects.toMatchObject({ status: 403, message });
    }

    expect(getAuthData()).toBe('live-admin');
    expect(clear).not.toHaveBeenCalled();
    expect(routerNavigate).not.toHaveBeenCalled();
  });

  it('drops the step-up grant on any auth identity change, not only the 403 teardown', () => {
    // The grant is bound server-side to (user_id, session_id); a stale header
    // after re-login would flip a would-be recent-password pass into a
    // spurious re-auth 403, so every logout/login path must clear it.
    setAuthData(null);
    setStepUpGrant('stale-grant', 60);
    setAuthData('fresh-admin');
    expect(getStepUpToken()).toBeNull();

    setStepUpGrant('second-grant', 60);
    setAuthData(null);
    expect(getStepUpToken()).toBeNull();
  });

  it('drops the step-up grant when another tab changes the session', () => {
    setupAuthSync();
    setStepUpGrant('cross-tab-grant', 60);

    window.dispatchEvent(new StorageEvent('storage', { key: AUTH_KEY, newValue: null }));

    expect(getStepUpToken()).toBeNull();
  });

  it('closes the re-auth prompt when a session-expiry 403 tears the session down', async () => {
    // A stranded password modal over /login is a dead end: stepUp can never
    // succeed without a live session.
    setAuthData('expired-admin');
    maybePromptStepUp(new ApiError(403, 'Recent password verification is required'));
    expect(isStepUpPromptRequested()).toBe(true);
    apiClient.axios.defaults.adapter = adapterFor(403, { message: 'auth required' });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403 });

    expect(isStepUpPromptRequested()).toBe(false);
  });

  it('sends the active step-up grant as the x-v2board-step-up header until it expires', async () => {
    vi.useFakeTimers();
    try {
      setStepUpGrant('step-token', 60);
      const seenHeaders: Array<Record<string, unknown> | undefined> = [];
      apiClient.axios.defaults.adapter = async (config) => {
        seenHeaders.push(config.headers as Record<string, unknown> | undefined);
        return { data: { data: true }, status: 200, statusText: 'OK', headers: {}, config };
      };

      await apiClient.request({ url: '/user/update', method: 'POST', responseSchema: z.unknown() });
      expect(seenHeaders[0]?.['x-v2board-step-up']).toBe('step-token');

      // Past expiry (minus the safety margin) the header must disappear: the
      // backend rejects a stale token instead of falling back to the
      // recent-password window.
      vi.advanceTimersByTime(61_000);
      await apiClient.request({ url: '/user/update', method: 'POST', responseSchema: z.unknown() });
      expect(seenHeaders[1]?.['x-v2board-step-up']).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
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

describe('admin explicit sign-out revocation', () => {
  let requests: Array<Parameters<AdapterFn>[0]>;

  beforeEach(() => {
    registerRouterNavigation({
      navigate: vi.fn<RouterNavigate>().mockResolvedValue(undefined),
    });
    registerSessionCacheClearer(() => undefined);
    setAuthData('live-admin-token');
    requests = [];
  });

  afterEach(() => {
    apiClient.axios.defaults.adapter = originalAdapter;
    registerSessionCacheClearer(() => undefined);
    setAuthData(null);
  });

  it('fires the revocation with the captured bearer and tears down even when it rejects', async () => {
    const reject = transportErrorAdapter('Network Error');
    apiClient.axios.defaults.adapter = async (config) => {
      requests.push(config);
      return reject(config);
    };

    signOut();

    // Local teardown is synchronous and never waits on (or fails with) the
    // network; the rejection is swallowed by the fire-and-forget call.
    expect(getAuthData()).toBeNull();

    await vi.waitFor(() => expect(requests).toHaveLength(1));
    // Admin sessions are ordinary user sessions to the backend; the shared
    // user endpoint revokes them too.
    expect(requests[0]?.url).toBe('/user/logout');
    expect(requests[0]?.method).toBe('post');
    // The bearer must be captured before teardown: the request interceptor
    // reads the auth store on a microtask, after it is already cleared.
    expect(requests[0]?.headers?.authorization).toBe('live-admin-token');
  });

  it('does not fire the revocation from the 403 session-expiry teardown', async () => {
    const forbidden = adapterFor(403, { message: 'auth required' });
    apiClient.axios.defaults.adapter = async (config) => {
      requests.push(config);
      return forbidden(config);
    };

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403 });
    expect(getAuthData()).toBeNull();

    // The token is already dead server-side; revoking here would only 403
    // again into the same handler. Let any stray fire-and-forget call surface
    // before asserting.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(requests.map((request) => request.url)).toEqual(['/user/info']);
  });
});
