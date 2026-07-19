import { QueryClient } from '@tanstack/react-query';
import type * as ApiClientModule from '@v2board/api-client';
import type { LoaderFunctionArgs } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ADMIN_LAYOUT_ROUTE_PATHS,
  ADMIN_ROUTE_PATHS,
  ADMIN_STANDALONE_ROUTE_PATHS,
  createAdminLoginLoader,
  createAdminRoutes,
  createRequireAdminMiddleware,
  normalizeAdminRouteLoader,
  rootAdminRouteLoader,
  unknownAdminRouteLoader,
} from './App';
import { getAuthData, setAuthData } from '@/lib/auth';
import { getAdminBasename } from '@/lib/runtime-config';
import { adminSessionKeys } from '@/lib/session-queries';

const mocks = vi.hoisted(() => ({
  checkLogin: vi.fn(),
  userInfo: vi.fn(),
}));

vi.mock('@v2board/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof ApiClientModule>();
  return {
    ...actual,
    user: {
      ...actual.user,
      checkLogin: mocks.checkLogin,
      info: mocks.userInfo,
    },
  };
});

// History routing (docs/api-dialect.md §10.1): loader request URLs carry the
// dynamic admin basename exactly as the browser router produces them, and
// getRequestRoutePath strips it back to the app-relative route path.
function requestFor(routePath: string): Request {
  return new Request(`https://v2board.local${getAdminBasename()}${routePath}`);
}

function loaderArgs(routePath: string): LoaderFunctionArgs {
  return {
    request: requestFor(routePath),
    params: {},
    context: {},
  } as unknown as LoaderFunctionArgs;
}

function queryClient(): QueryClient {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

async function expectRedirect(run: () => unknown | Promise<unknown>, location: string) {
  try {
    await run();
    throw new Error('expected loader to redirect');
  } catch (error) {
    expect(error).toBeInstanceOf(Response);
    expect((error as Response).headers.get('Location')).toBe(location);
  }
}

type AuthMiddleware = ReturnType<typeof createRequireAdminMiddleware>;

/** Invokes the auth middleware the way the router does on a navigation. The
 * gate relies on the router auto-advancing when next() is not called, so the
 * stub fails loudly if the middleware ever starts calling next() itself. */
function runAuthMiddleware(middleware: AuthMiddleware, routePath: string) {
  const next = () => {
    throw new Error('auth middleware must not call next()');
  };
  return middleware(
    loaderArgs(routePath) as unknown as Parameters<AuthMiddleware>[0],
    next as Parameters<AuthMiddleware>[1],
  );
}

describe('admin data router', () => {
  beforeEach(() => {
    localStorage.clear();
    mocks.checkLogin.mockReset();
    mocks.userInfo.mockReset();
  });

  afterEach(() => {
    setAuthData(null);
  });

  it('preserves every externally visible route path', () => {
    expect([...ADMIN_ROUTE_PATHS]).toEqual([
      '/audit',
      '/config/payment',
      '/config/system',
      '/coupon',
      '/giftcard',
      '/dashboard',
      '/',
      '/knowledge',
      '/login',
      '/notice',
      '/order',
      '/plan',
      '/queue',
      '/server/group',
      '/server/manage',
      '/server/route',
      '/ticket/:ticket_id',
      '/ticket',
      '/user',
    ]);
    expect([...ADMIN_STANDALONE_ROUTE_PATHS]).toEqual(['/ticket/:ticket_id']);
    expect(ADMIN_LAYOUT_ROUTE_PATHS).not.toContain('/ticket/:ticket_id');
  });

  it('uses lazy route modules for every page instead of eager page imports', () => {
    const routes = createAdminRoutes(new QueryClient());
    const children = routes[0]?.children ?? [];
    const protectedBranch = children.find((route) => (route.middleware?.length ?? 0) > 0);
    const layout = protectedBranch?.children?.find((route) =>
      route.children?.some((child) => child.path === '/dashboard'),
    );
    const dashboard = layout?.children?.find((route) => route.path === '/dashboard');
    const login = children.find((route) => route.path === '/login');

    // One pathless middleware branch gates every protected page (layer 1).
    expect(children.filter((route) => (route.middleware?.length ?? 0) > 0)).toHaveLength(1);
    expect(protectedBranch?.middleware).toHaveLength(1);
    expect(protectedBranch?.children?.some((route) => route.path === '/ticket/:ticket_id')).toBe(
      true,
    );
    expect(dashboard?.lazy).toBeTypeOf('function');
    expect(login?.lazy).toBeTypeOf('function');
    expect(login?.loader).toBeTypeOf('function');
    expect(dashboard?.element).toBeUndefined();
    expect(login?.element).toBeUndefined();
  });

  it('redirects unauthenticated protected navigations before a page module renders', async () => {
    const middleware = createRequireAdminMiddleware(new QueryClient());
    await expectRedirect(() => runAuthMiddleware(middleware, '/order'), '/login?redirect=%2Forder');
    expect(mocks.checkLogin).not.toHaveBeenCalled();
  });

  it('rejects a stored non-admin identity and destroys its credential', async () => {
    setAuthData('regular-user-token');
    mocks.checkLogin.mockResolvedValue({ is_login: true, is_admin: false });
    const middleware = createRequireAdminMiddleware(new QueryClient());

    await expectRedirect(() => runAuthMiddleware(middleware, '/dashboard'), '/login');
    expect(getAuthData()).toBeNull();
  });

  it('allows a verified admin only after resolving the shell identity', async () => {
    setAuthData('admin-token');
    mocks.checkLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({ email: 'admin@example.com' });
    const middleware = createRequireAdminMiddleware(new QueryClient());

    // Resolving without a value lets the router auto-advance to the loaders.
    await expect(runAuthMiddleware(middleware, '/dashboard')).resolves.toBeUndefined();
    expect(mocks.userInfo).toHaveBeenCalledOnce();
  });

  it('lets the route boundary own a shell-identity failure', async () => {
    setAuthData('admin-token');
    const failure = new Error('user info offline');
    mocks.checkLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockRejectedValue(failure);
    const middleware = createRequireAdminMiddleware(queryClient());

    await expect(runAuthMiddleware(middleware, '/dashboard')).rejects.toBe(failure);
    expect(getAuthData()).toBe('admin-token');
  });

  it('returns a safe login target without probing when no credential exists', async () => {
    const loader = createAdminLoginLoader(queryClient());

    await expect(loader(loaderArgs('/login?redirect=%2Forder%3Fstatus%3D0'))).resolves.toEqual({
      redirectTarget: '/order?status=0',
    });
    for (const target of ['order', 'https://evil.example', '//evil.example', '/\\evil.example']) {
      const encoded = encodeURIComponent(target);
      await expect(loader(loaderArgs(`/login?redirect=${encoded}`))).resolves.toEqual({
        redirectTarget: '/dashboard',
      });
    }
    expect(mocks.checkLogin).not.toHaveBeenCalled();
  });

  it('deduplicates an existing-admin probe and redirects from the loader', async () => {
    setAuthData('admin-token');
    mocks.checkLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({ email: 'admin@example.com' });
    const client = queryClient();
    const loader = createAdminLoginLoader(client);

    await Promise.all([
      expectRedirect(() => loader(loaderArgs('/login?redirect=%2Forder')), '/order'),
      expectRedirect(() => loader(loaderArgs('/login?redirect=%2Forder')), '/order'),
    ]);
    await expect(
      runAuthMiddleware(createRequireAdminMiddleware(client), '/order'),
    ).resolves.toBeUndefined();

    expect(mocks.checkLogin).toHaveBeenCalledOnce();
    await vi.waitFor(() => expect(mocks.userInfo).toHaveBeenCalledOnce());
    await vi.waitFor(() =>
      expect(client.getQueryData(adminSessionKeys.userInfo)).toEqual({
        email: 'admin@example.com',
      }),
    );
  });

  it('clears a verified non-admin token and leaves the login form available', async () => {
    setAuthData('regular-user-token');
    mocks.checkLogin.mockResolvedValue({ is_login: true, is_admin: false });
    const loader = createAdminLoginLoader(queryClient());

    await expect(loader(loaderArgs('/login?redirect=%2Fticket'))).resolves.toEqual({
      redirectTarget: '/ticket',
    });
    expect(getAuthData()).toBeNull();
    expect(mocks.userInfo).not.toHaveBeenCalled();
  });

  it('lets the route boundary own a real session-probe failure', async () => {
    setAuthData('temporarily-unverifiable-token');
    const failure = new Error('offline');
    mocks.checkLogin.mockRejectedValue(failure);
    const loader = createAdminLoginLoader(queryClient());

    await expect(loader(loaderArgs('/login?redirect=%2Fplan'))).rejects.toBe(failure);
    expect(getAuthData()).toBe('temporarily-unverifiable-token');
    expect(mocks.userInfo).not.toHaveBeenCalled();
  });

  it('settles an aborted login probe without presenting a stale route error', async () => {
    setAuthData('admin-token');
    mocks.checkLogin.mockRejectedValue(new DOMException('aborted', 'AbortError'));
    const loader = createAdminLoginLoader(queryClient());

    await expect(loader(loaderArgs('/login?redirect=%2Fplan'))).resolves.toEqual({
      redirectTarget: '/plan',
    });
    expect(getAuthData()).toBe('admin-token');
  });

  it('normalizes malformed protected paths without bypassing session routing', async () => {
    await expectRedirect(() => normalizeAdminRouteLoader(loaderArgs('/login/dashboard')), '/login');

    setAuthData('admin-token');
    await expectRedirect(
      () => normalizeAdminRouteLoader(loaderArgs('/login/dashboard')),
      '/dashboard',
    );
  });

  it('routes root and unknown URLs according to the current session', async () => {
    await expectRedirect(() => rootAdminRouteLoader(), '/login');
    await expectRedirect(() => unknownAdminRouteLoader(loaderArgs('/does-not-exist')), '/login');

    setAuthData('admin-token');
    await expectRedirect(() => rootAdminRouteLoader(), '/dashboard');
  });
});
