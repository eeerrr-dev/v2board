import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getNormalizedLegacyHashPath,
  installLegacyDevModuleRecovery,
  installLegacyDevWhiteScreenFallback,
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
    expect(
      getNormalizedLegacyHashPath('/order/2026060408061914022260977/profile', appOptions),
    ).toBe('/profile');
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

  it('preserves router history state when correcting pushed legacy hashes', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);
    const routerState = { idx: 3, key: 'legacy', usr: null };

    window.history.pushState(routerState, '', '/#/login/dashboard');

    expect(window.location.hash).toBe('#/dashboard');
    expect(window.history.state).toEqual(routerState);
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

  it('notifies hash listeners when a pushed bad hash is corrected after mount', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const listener = vi.fn();
    window.addEventListener('hashchange', listener);
    const dispose = installLegacyHashRouteNormalizer(options);

    window.history.pushState(null, '', '/#/login/dashboard');

    expect(window.location.hash).toBe('#/dashboard');
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener.mock.calls[0]?.[0]).toMatchObject({
      oldURL: expect.stringContaining('/#/login/dashboard'),
      newURL: expect.stringContaining('/#/dashboard'),
    });
    dispose();
    window.removeEventListener('hashchange', listener);
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

  it('converts same-origin legacy pathname anchors to hash navigation', () => {
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML = '<a id="legacy-link" href="/ticket/7/dashboard">Legacy</a>';
    const listener = vi.fn();
    window.addEventListener('hashchange', listener);
    const dispose = installLegacyHashRouteNormalizer({
      ...options,
      nestedPrefixes: options.routes,
    });
    const anchor = document.getElementById('legacy-link')!;
    const event = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 });

    const notCancelled = anchor.dispatchEvent(event);

    expect(notCancelled).toBe(false);
    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/dashboard');
    expect(listener).toHaveBeenCalled();
    dispose();
    window.removeEventListener('hashchange', listener);
  });

  it('converts same-origin hash anchors to the canonical path', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/login/dashboard#/ticket');
    document.body.innerHTML = '<a id="legacy-link" href="/#/ticket/7">Legacy</a>';
    const dispose = installLegacyHashRouteNormalizer(options);
    const anchor = document.getElementById('legacy-link')!;

    anchor.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }));

    expect(window.location.pathname).toBe('/');
    expect(window.location.hash).toBe('#/ticket/7');
    dispose();
  });

  it('leaves external and new-window anchors alone', () => {
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML = [
      '<a id="external" href="https://example.com/dashboard">External</a>',
      '<a id="blank" href="/ticket" target="_blank">Blank</a>',
    ].join('');
    const dispose = installLegacyHashRouteNormalizer(options);

    const external = document.getElementById('external')!;
    const blank = document.getElementById('blank')!;
    const externalEvent = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 });
    const blankEvent = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 });

    expect(external.dispatchEvent(externalEvent)).toBe(true);
    expect(blank.dispatchEvent(blankEvent)).toBe(true);
    expect(window.location.hash).toBe('');
    dispose();
  });

  it('stops converting legacy anchor clicks after cleanup', () => {
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML = '<a id="legacy-link" href="/ticket">Legacy</a>';
    const dispose = installLegacyHashRouteNormalizer(options);
    dispose();

    const anchor = document.getElementById('legacy-link')!;
    const event = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 });

    expect(anchor.dispatchEvent(event)).toBe(true);
    expect(window.location.hash).toBe('');
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

  it('waits one check before recovering an empty legacy layout main container', () => {
    vi.useFakeTimers();
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML =
      '<div id="root"><div id="page-container"><nav>仪表盘</nav><header>admin@local</header><main id="main-container"><div class="content content-full"></div></main></div></div>';
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 126,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).toBeNull();

    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '126');
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('recovers a persistent blank legacy layout main container under React control', () => {
    vi.useFakeTimers();
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML =
      '<div id="root"><div id="page-container"><nav>仪表盘</nav><header>admin@local</header><main id="main-container"><div class="content content-full"></div></main></div></div>';
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      now: () => 125,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);
    expect(replace).not.toHaveBeenCalled();

    vi.advanceTimersByTime(10);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_recover', '125');
    expect(replace).toHaveBeenCalledWith(expected.toString());
    expect(document.querySelector('#main-container')?.textContent).not.toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).toBeNull();
    dispose();
  });

  it('cancels blank main recovery when route content appears before the second check', () => {
    vi.useFakeTimers();
    window.localStorage.setItem('authorization', 'jwt');
    document.body.innerHTML =
      '<div id="root"><div id="page-container"><nav>仪表盘</nav><header>admin@local</header><main id="main-container"><div class="content content-full"></div></main></div></div>';
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);
    document.querySelector('#main-container .content')!.textContent = '仪表盘';
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    dispose();
  });

  it('does not recover a legacy main container that still has loading controls', () => {
    vi.useFakeTimers();
    document.body.innerHTML =
      '<div id="root"><main id="main-container"><div class="spinner-grow text-primary"><span class="sr-only">Loading...</span></div></main></div>';
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

  it('renders a visible fallback when the guest fallback remains empty', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/login');
    window.sessionStorage.setItem('v2board:white-screen-recovery:/#/login', '2');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).not.toBeNull();
    dispose();
  });

  it('renders a visible fallback inside a persistently blank mounted legacy main container', () => {
    vi.useFakeTimers();
    document.body.innerHTML =
      '<div id="root"><div id="page-container"><nav>仪表盘</nav><header>admin@local</header><main id="main-container"><div class="content content-full"></div></main></div></div>';
    setUrl('/#/login');
    window.sessionStorage.setItem('v2board:white-screen-recovery:/#/login', '2');
    const replace = vi.fn();
    const dispose = installLegacyWhiteScreenRecovery(options, {
      delay: 10,
      replace,
    });

    window.dispatchEvent(new HashChangeEvent('hashchange'));
    vi.advanceTimersByTime(10);
    vi.advanceTimersByTime(10);

    expect(replace).not.toHaveBeenCalled();
    expect(document.querySelector('#page-container')).not.toBeNull();
    expect(document.querySelector('#main-container')?.textContent).toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).not.toBeNull();
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

  it('retries stale Vite optimized dependency reloads up to the configured limit', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    let timestamp = 654;
    const dispose = installLegacyDevModuleRecovery({
      maxAttempts: 2,
      now: () => timestamp++,
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
    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'Outdated Optimize Dep',
        filename: 'http://127.0.0.1:5174/node_modules/.vite/deps/react-router-dom.js?v=old',
      }),
    );

    const first = new URL(window.location.href);
    first.searchParams.set('__v2board_dev_recover', '654');
    const second = new URL(window.location.href);
    second.searchParams.set('__v2board_dev_recover', '655');
    expect(replace).toHaveBeenCalledTimes(2);
    expect(replace).toHaveBeenNthCalledWith(1, first.toString());
    expect(replace).toHaveBeenNthCalledWith(2, second.toString());
    dispose();
  });

  it('recovers Vite optimized module export mismatch errors', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      maxAttempts: 1,
      now: () => 432,
      replace,
    });

    window.dispatchEvent(
      new ErrorEvent('error', {
        message:
          "The requested module '/node_modules/.vite/deps/antd.js?v=old' does not provide an export named 'App'",
      }),
    );

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '432');
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('recovers stale Vite preload errors carried in CustomEvent detail', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      maxAttempts: 1,
      now: () => 765,
      replace,
    });

    window.dispatchEvent(
      new CustomEvent('vite:preloadError', {
        detail: new Error(
          'Failed to fetch dynamically imported module: http://127.0.0.1:5174/src/pages/orders.tsx?t=old',
        ),
      }),
    );

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '765');
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('recovers stale optimized module errors carried in Vite error details', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      maxAttempts: 1,
      now: () => 876,
      replace,
    });

    window.dispatchEvent(
      new CustomEvent('vite:error', {
        detail: {
          err: {
            message:
              "The requested module '/node_modules/.vite/deps/antd.js?v=old' does not provide an export named 'App'",
          },
        },
      }),
    );

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '876');
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('recovers browser-specific dynamic import failures from promise rejections', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      maxAttempts: 1,
      now: () => 246,
      replace,
    });
    const event = Object.assign(new Event('unhandledrejection'), {
      reason: new Error(
        'error loading dynamically imported module: http://127.0.0.1:5174/src/pages/users.tsx?t=old',
      ),
    });

    window.dispatchEvent(event);

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '246');
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('recovers chunk and export mismatch failures nested under error causes', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      maxAttempts: 1,
      now: () => 357,
      replace,
    });
    const error = new Error('ChunkLoadError: Loading chunk admin-users failed');
    (error as Error & { cause?: Error }).cause = new Error(
      "The requested module '/assets/index-old.js' doesn't provide an export named 'default'",
    );

    window.dispatchEvent(new ErrorEvent('error', { error }));

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '357');
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith(expected.toString());
    dispose();
  });

  it('clears stale Vite module recovery attempts after route content renders', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"><main>仪表盘</main></div>';
    setUrl('/#/dashboard');
    window.sessionStorage.setItem('v2board:dev-module-recovery:/#/dashboard', '3');

    const dispose = installLegacyDevModuleRecovery({ clearDelay: 10 });
    vi.advanceTimersByTime(10);

    expect(window.sessionStorage.getItem('v2board:dev-module-recovery:/#/dashboard')).toBeNull();
    dispose();
  });

  it('keeps stale Vite module recovery attempts while the route is still blank', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/dashboard');
    window.sessionStorage.setItem('v2board:dev-module-recovery:/#/dashboard', '3');

    const dispose = installLegacyDevModuleRecovery({ clearDelay: 10 });
    vi.advanceTimersByTime(10);

    expect(window.sessionStorage.getItem('v2board:dev-module-recovery:/#/dashboard')).toBe('3');
    dispose();
  });

  it('recovers stale Vite modules again after a successful render clears old attempts', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"><main>仪表盘</main></div>';
    setUrl('/#/dashboard');
    window.sessionStorage.setItem('v2board:dev-module-recovery:/#/dashboard', '3');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({
      clearDelay: 10,
      now: () => 159,
      replace,
    });

    vi.advanceTimersByTime(10);
    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'Outdated Optimize Dep',
        filename: 'http://127.0.0.1:5174/node_modules/.vite/deps/antd.js?v=old',
      }),
    );

    const expected = new URL(window.location.href);
    expected.searchParams.set('__v2board_dev_recover', '159');
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

  it('does not reload ordinary runtime errors just because the stack points at Vite deps', () => {
    setUrl('/#/dashboard');
    const replace = vi.fn();
    const dispose = installLegacyDevModuleRecovery({ replace });

    window.dispatchEvent(
      new ErrorEvent('error', {
        message: 'Cannot read properties of undefined',
        filename: 'http://127.0.0.1:5174/node_modules/.vite/deps/react-dom_client.js?v=old',
        error: new Error(
          'Cannot read properties of undefined\n    at renderWithHooks (http://127.0.0.1:5174/node_modules/.vite/deps/react-dom_client.js?v=old:4213:1)',
        ),
      }),
    );

    expect(replace).not.toHaveBeenCalled();
    dispose();
  });

  it('renders a visible dev fallback when the React root stays empty', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"></div>';
    setUrl('/#/dashboard');
    const dispose = installLegacyDevWhiteScreenFallback({ delay: 10 });

    vi.advanceTimersByTime(10);

    expect(document.body.textContent).toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).not.toBeNull();
    dispose();
  });

  it('renders the dev fallback when only the legacy main container stays empty', () => {
    vi.useFakeTimers();
    document.body.innerHTML =
      '<div id="root"><div id="page-container"><nav>仪表盘</nav><header>admin@local</header><main id="main-container"><div class="content content-full"></div></main></div></div>';
    setUrl('/#/dashboard');
    const dispose = installLegacyDevWhiteScreenFallback({ delay: 10 });

    vi.advanceTimersByTime(10);

    expect(document.querySelector('#page-container')).not.toBeNull();
    expect(document.querySelector('#main-container')?.textContent).not.toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).toBeNull();

    vi.advanceTimersByTime(10);

    expect(document.querySelector('#main-container')?.textContent).toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).not.toBeNull();
    dispose();
  });

  it('does not render the dev fallback once the React root has content', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="root"><main>仪表盘</main></div>';
    setUrl('/#/dashboard');
    const dispose = installLegacyDevWhiteScreenFallback({ delay: 10 });

    vi.advanceTimersByTime(10);

    expect(document.body.textContent).not.toContain('页面加载失败');
    expect(document.querySelector('[data-v2board-white-screen-fallback="1"]')).toBeNull();
    dispose();
  });
});
