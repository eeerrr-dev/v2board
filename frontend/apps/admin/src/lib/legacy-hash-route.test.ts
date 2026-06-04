import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getNormalizedLegacyHashPath,
  installLegacyHashRouteNormalizer,
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
    window.localStorage.clear();
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
});
