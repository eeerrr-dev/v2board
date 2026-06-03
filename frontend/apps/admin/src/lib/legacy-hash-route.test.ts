import { afterEach, describe, expect, it } from 'vitest';
import {
  installLegacyHashRouteNormalizer,
  normalizeLegacyHashRoute,
} from '@v2board/config';

const options = {
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  publicRoutes: ['/login'],
  routes: ['/dashboard', '/login', '/ticket/:ticket_id'],
} as const;

function setUrl(url: string) {
  window.history.replaceState(null, '', url);
}

describe('normalizeLegacyHashRoute', () => {
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

    expect(window.location.pathname).toBe('/login/dashboard');
    expect(window.location.hash).toBe('#/login');
  });

  it('keeps dynamic detail routes as known routes', () => {
    window.localStorage.setItem('authorization', 'jwt');
    setUrl('/#/ticket/7');

    normalizeLegacyHashRoute(options);

    expect(window.location.hash).toBe('#/ticket/7');
  });

  it('normalizes broken nested hashes that appear after the app has mounted', () => {
    window.localStorage.setItem('authorization', 'jwt');
    const dispose = installLegacyHashRouteNormalizer(options);
    setUrl('/#/login/dashboard');

    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(window.location.hash).toBe('#/dashboard');
    dispose();
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
