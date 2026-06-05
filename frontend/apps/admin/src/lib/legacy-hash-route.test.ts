import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getNormalizedLegacyHashPath,
  installLegacyDevModuleRecovery,
  installLegacyHashRouteNormalizer,
  installLegacyWhiteScreenRecovery,
  normalizeLegacyHashRoute,
} from '@v2board/config';

const options = {
  authenticatedFallback: '/dashboard',
  canonicalPath: '/',
  guestFallback: '/login',
  publicRoutes: ['/login'],
  routes: ['/dashboard', '/login', '/ticket/:ticket_id', '/ticket'],
} as const;

function setUrl(url: string) {
  window.history.replaceState(null, '', url);
}

describe('normalizeLegacyHashRoute', () => {
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
    vi.useRealTimers();
    window.localStorage.clear();
    window.sessionStorage.clear();
    document.body.innerHTML = '';
    setUrl('/');
  });

  it('normalizes nested login hashes to the authenticated destination', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/#/login/dashboard');

    normalizeLegacyHashRoute(options);

    expect(window.location.hash).toBe('#/dashboard');
  });

  it('normalizes nested login hashes to login when there is no auth token', () => {
    setUrl('/#/login/dashboard');

    normalizeLegacyHashRoute(options);

    expect(window.location.hash).toBe('#/login');
  });

  it('normalizes non-hash legacy paths before HashRouter reads them', () => {
    setUrl('/login/dashboard');

    normalizeLegacyHashRoute(options);

    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/login');
  });

  it('normalizes authenticated public hashes to the authenticated destination', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/#/login');

    normalizeLegacyHashRoute(options);

    expect(window.location.hash).toBe('#/dashboard');
  });

  it('normalizes authenticated root entries before rendering the empty root shell', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/');

    normalizeLegacyHashRoute({
      ...options,
      publicRoutes: ['/', '/login'],
      routes: ['/', '/dashboard', '/login'],
    });

    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/dashboard');
  });

  it('cleans stale legacy pathnames when the hash route is already valid', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/login/dashboard#/dashboard');

    normalizeLegacyHashRoute(options);

    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/dashboard');
  });

  it('keeps dynamic detail routes as known routes', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/#/ticket/7');

    normalizeLegacyHashRoute(options);

    expect(window.location.hash).toBe('#/ticket/7');
  });

  it('keeps exact dynamic detail routes before recovering nested route prefixes', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const appRoutes = [
      '/dashboard',
      '/login',
      '/order/:trade_no',
      '/order',
      '/plan/:plan_id',
      '/plan',
      '/profile',
      '/ticket/:ticket_id',
      '/ticket',
    ] as const;
    const appOptions = {
      ...options,
      nestedPrefixes: appRoutes,
      routes: appRoutes,
    };

    expect(getNormalizedLegacyHashPath('/order/2026060408061914022260977', appOptions)).toBe(
      '/order/2026060408061914022260977',
    );
    expect(getNormalizedLegacyHashPath('/plan/8', appOptions)).toBe('/plan/8');
    expect(getNormalizedLegacyHashPath('/ticket/7', appOptions)).toBe('/ticket/7');
    expect(getNormalizedLegacyHashPath('/order/2026060408061914022260977/profile', appOptions)).toBe(
      '/profile',
    );
  });

  it('recovers known pages that would otherwise be swallowed by dynamic detail routes', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const appRoutes = [
      '/dashboard',
      '/login',
      '/order/:trade_no',
      '/order',
      '/plan/:plan_id',
      '/plan',
      '/profile',
      '/server/manage',
      '/ticket/:ticket_id',
      '/ticket',
    ] as const;
    const appOptions = {
      ...options,
      nestedPrefixes: appRoutes,
      routes: appRoutes,
    };

    expect(getNormalizedLegacyHashPath('/plan/order', appOptions)).toBe('/order');
    expect(getNormalizedLegacyHashPath('/ticket/dashboard', appOptions)).toBe('/dashboard');
    expect(getNormalizedLegacyHashPath('/server/manage/order', appOptions)).toBe('/order');
  });

  it('normalizes broken nested hashes that appear after the app has mounted', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);
    setUrl('/#/login/dashboard');

    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(window.location.hash).toBe('#/dashboard');
    dispose();
  });

  it('normalizes browser history moves after the app has mounted', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);
    setUrl('/#/login/dashboard');

    window.dispatchEvent(new PopStateEvent('popstate'));

    expect(window.location.hash).toBe('#/dashboard');
    dispose();
  });

  it('normalizes router pushState hash changes after the app has mounted', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);

    window.history.pushState(null, '', '/#/login/dashboard');

    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/dashboard');
    dispose();
  });

  it('notifies listeners when a pushState URL is corrected after mount', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const listener = vi.fn();
    window.addEventListener('popstate', listener);
    const dispose = installLegacyHashRouteNormalizer(options);

    window.history.pushState(null, '', '/#/login/dashboard');

    expect(window.location.hash).toBe('#/dashboard');
    expect(listener).toHaveBeenCalledTimes(1);
    dispose();
    window.removeEventListener('popstate', listener);
  });

  it('does not notify listeners for already normalized pushState URLs', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const listener = vi.fn();
    window.addEventListener('popstate', listener);
    const dispose = installLegacyHashRouteNormalizer(options);

    window.history.pushState(null, '', '/#/dashboard');

    expect(window.location.hash).toBe('#/dashboard');
    expect(listener).not.toHaveBeenCalled();
    dispose();
    window.removeEventListener('popstate', listener);
  });

  it('normalizes router replaceState hash changes after the app has mounted', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);

    window.history.replaceState(null, '', '/login/dashboard#/ticket/7/dashboard');

    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/dashboard');
    dispose();
  });

  it('normalizes internal router locations without waiting for hashchange', () => {
    window.localStorage.setItem('authorization', 'jwt');

    expect(getNormalizedLegacyHashPath('/login/dashboard', options)).toBe('/dashboard');
    expect(getNormalizedLegacyHashPath('/ticket/7', options)).toBe('/ticket/7');
  });

  it('normalizes paths nested below any known route prefix when configured', () => {
    window.localStorage.setItem('authorization', 'jwt');

    expect(
      getNormalizedLegacyHashPath('/dashboard/ticket/7', {
        ...options,
        nestedPrefixes: options.routes,
      }),
    ).toBe('/ticket/7');
  });

  it('recovers paths nested below multiple known route prefixes', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const appRoutes = [
      '/config/system',
      '/dashboard',
      '/login',
      '/order',
      '/server/manage',
      '/ticket/:ticket_id',
      '/ticket',
    ] as const;
    const appOptions = {
      ...options,
      nestedPrefixes: appRoutes,
      routes: appRoutes,
    };

    expect(getNormalizedLegacyHashPath('/login/dashboard/config/system', appOptions)).toBe(
      '/config/system',
    );
    expect(getNormalizedLegacyHashPath('/dashboard/config/system/server/manage', appOptions)).toBe(
      '/server/manage',
    );
    expect(getNormalizedLegacyHashPath('/ticket/7/dashboard/order', appOptions)).toBe('/order');
  });

  it('prefers dynamic route prefixes over shorter static prefixes when recovering nested paths', () => {
    window.localStorage.setItem('authorization', 'jwt');

    expect(
      getNormalizedLegacyHashPath('/ticket/7/dashboard', {
        ...options,
        nestedPrefixes: [...options.routes, '/ticket'],
      }),
    ).toBe('/dashboard');
  });

  it('stops normalizing runtime hash changes after cleanup', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);
    dispose();
    setUrl('/#/login/dashboard');

    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(window.location.hash).toBe('#/login/dashboard');
  });

  it('normalizes a broken empty route before reloading the same blank URL', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/login/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 123,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '123');
    expected.hash = '#/login';
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('reloads once when the React root is empty on an already normalized route', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/login');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 123,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '123');
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('reloads when React leaves a blank shell in the root', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"><div></div></div>';
    setUrl('/#/login');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 124,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '124');
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('does not recover icon-only loading or control shells as blank pages', () => {
    vi.useFakeTimers();
    document.body.innerHTML =
      '<div id="root"><button aria-label="loading"></button><i class="fa fa-spinner"></i></div>';
    setUrl('/#/login');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    dispose();
  });

  it('falls back to the authenticated route when the same empty route is recovered twice', () => {
    vi.useFakeTimers();
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/login/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 456,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '456');
    expected.hash = '#/dashboard';
    expect(replace).toHaveBeenLastCalledWith(expected.toString());
    dispose();
  });

  it('still falls back when a stale empty-route recovery count is already exhausted', () => {
    vi.useFakeTimers();
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/login/dashboard');
    window.sessionStorage.setItem('v2board:white-screen-recovery:/#/login/dashboard', '2');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 789,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '789');
    expected.hash = '#/dashboard';
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('clears the current route recovery count after the React root renders content', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"><main>仪表盘</main></div>';
    setUrl('/#/dashboard');
    window.sessionStorage.setItem('v2board:white-screen-recovery:/#/dashboard', '2');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    expect(window.sessionStorage.getItem('v2board:white-screen-recovery:/#/dashboard')).toBeNull();
    dispose();
  });

  it('falls back to login and clears auth when the authenticated fallback remains empty', () => {
    vi.useFakeTimers();
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/dashboard');
    window.sessionStorage.setItem('v2board:white-screen-recovery:/#/dashboard', '2');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 987,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '987');
    expected.hash = '#/login';
    expect(window.localStorage.getItem('authorization')).toBeNull();
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('does not recover when the React root has rendered content', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"><main>仪表盘</main></div>';
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    dispose();
  });

  it('reloads once when a stale Vite optimized dependency fails to load', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      now: () => 654,
      replace,
    });

    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'Outdated Optimize Dep',
        filename: 'http://127.0.0.1:5174/node_modules/.vite/deps/react-router-dom.js?v=old',
      }),
    );
    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'Outdated Optimize Dep',
        filename: 'http://127.0.0.1:5174/node_modules/.vite/deps/react-router-dom.js?v=old',
      }),
    );

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '654');
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('does not reload for ordinary runtime errors', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({ replace });

    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'Cannot read properties of undefined',
        filename: 'http://127.0.0.1:5174/src/pages/dashboard.tsx',
      }),
    );

    expect(replace).not.toHaveBeenCalled();
    dispose();
  });
});
