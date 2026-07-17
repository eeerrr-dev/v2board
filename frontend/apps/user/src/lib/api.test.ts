import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { apiClient, signOut } from './api';
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

// The wire body Rust emits for a dead/expired/absent session on every
// internal route (docs/api-dialect.md §3.2).
const sessionExpiredProblem = {
  type: 'about:blank',
  title: 'Unauthorized',
  status: 401,
  code: 'session_expired',
  detail: '未登录或登陆已过期',
};

describe('user api unauthorized handling', () => {
  const originalAdapter = apiClient.axios.defaults.adapter;
  let clearSessionSpy: ReturnType<typeof vi.fn<() => void>>;
  let routerNavigate: ReturnType<typeof vi.fn<RouterNavigate>>;

  beforeEach(() => {
    routerNavigate = vi.fn<RouterNavigate>().mockResolvedValue(undefined);
    registerRouterNavigation({ navigate: routerNavigate });
    clearSessionSpy = vi.fn<() => void>();
    registerSessionCacheClearer(clearSessionSpy);
    setAuthData('token-401');
    clearSessionSpy.mockClear();
  });

  afterEach(() => {
    apiClient.axios.defaults.adapter = originalAdapter;
    registerSessionCacheClearer(() => undefined);
    setAuthData(null);
  });

  it('permanently clears the credential and redirects on the 401 session_expired problem', async () => {
    apiClient.axios.defaults.adapter = adapterFor(401, sessionExpiredProblem);

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 401, code: 'session_expired' });

    expect(getAuthData()).toBeNull();
    expect(routerNavigate).toHaveBeenCalledOnce();
    expect(routerNavigate).toHaveBeenCalledWith('/login', { replace: true });
    expect(clearSessionSpy).toHaveBeenCalledTimes(1);
  });

  it('keeps the session on a 403 problem: authorization verdicts never tear down', async () => {
    apiClient.axios.defaults.adapter = adapterFor(403, {
      type: 'about:blank',
      title: 'Forbidden',
      status: 403,
      code: 'permission_denied',
      detail: 'Permission denied',
    });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403, code: 'permission_denied' });

    expect(getAuthData()).toBe('token-401');
    expect(routerNavigate).not.toHaveBeenCalled();
    expect(clearSessionSpy).not.toHaveBeenCalled();
  });

  it('rejects a non-200 envelope code carried over HTTP 200 without touching the session', async () => {
    // Rust delivers auth failures as real HTTP statuses; an in-body code is a
    // parity-fixture shape and never a session verdict.
    apiClient.axios.defaults.adapter = adapterFor(200, {
      code: 403,
      data: null,
      message: 'auth required',
    });

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403 });

    expect(getAuthData()).toBe('token-401');
    expect(routerNavigate).not.toHaveBeenCalled();
    expect(clearSessionSpy).not.toHaveBeenCalled();
  });

  it('keeps concurrent session-expiry teardown idempotent', async () => {
    apiClient.axios.defaults.adapter = adapterFor(401, sessionExpiredProblem);

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

describe('explicit sign-out revocation', () => {
  const originalAdapter = apiClient.axios.defaults.adapter;
  let requests: Array<Parameters<AdapterFn>[0]>;

  beforeEach(() => {
    registerRouterNavigation({
      navigate: vi.fn<RouterNavigate>().mockResolvedValue(undefined),
    });
    registerSessionCacheClearer(() => undefined);
    setAuthData('live-token');
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
    expect(requests[0]?.url).toBe('/auth/session');
    expect(requests[0]?.method).toBe('delete');
    // The raw auth_data must be captured before teardown: the request
    // interceptor reads the auth store on a microtask, after it is already
    // cleared. The endpoint puts the Bearer scheme on the wire (§4.2).
    expect(requests[0]?.headers?.authorization).toBe('Bearer live-token');
  });

  it('does not fire the revocation from the 401 session-expiry teardown', async () => {
    const unauthorized = adapterFor(401, sessionExpiredProblem);
    apiClient.axios.defaults.adapter = async (config) => {
      requests.push(config);
      return unauthorized(config);
    };

    await expect(
      apiClient.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 401, code: 'session_expired' });
    expect(getAuthData()).toBeNull();

    // The token is already dead server-side; revoking here would only 401
    // again into the same handler. Let any stray fire-and-forget call surface
    // before asserting.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(requests.map((request) => request.url)).toEqual(['/user/info']);
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
