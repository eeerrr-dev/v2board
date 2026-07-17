import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { matchPath } from 'react-router';
import {
  applyLegacyHashRedirect,
  getNormalizedRoutePath,
  legacyHashHistoryUrl,
  stripBasePath,
  type RouteGuardOptions,
} from '@v2board/config';

const options: RouteGuardOptions = {
  matchRoute: (route, path, end) => matchPath({ path: route, end }, path),
  authStorageKey: 'authorization',
  authenticatedFallback: '/dashboard',
  authenticatedPublicFallbackRoutes: [],
  guestFallback: '/login',
  nestedPrefixes: ['/dashboard', '/ticket/:ticket_id'],
  publicRoutes: ['/', '/login', '/register'],
  routes: ['/', '/login', '/register', '/dashboard', '/ticket', '/ticket/:ticket_id'],
};

describe('route-guard contract adapter', () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.history.replaceState(null, '', '/');
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('preserves public routes and query strings for guests', () => {
    expect(getNormalizedRoutePath('/register?invite=abc', options)).toBe('/register?invite=abc');
  });

  it('redirects protected and unknown routes according to authentication', () => {
    expect(getNormalizedRoutePath('/dashboard', options)).toBe('/login');
    expect(getNormalizedRoutePath('/unknown', options)).toBe('/login');

    window.localStorage.setItem('authorization', 'token');
    expect(getNormalizedRoutePath('/dashboard', options)).toBe('/dashboard');
    expect(getNormalizedRoutePath('/unknown', options)).toBe('/dashboard');
    expect(getNormalizedRoutePath('/login', options)).toBe('/login');
  });

  it('recognizes dynamic routes and recovers duplicated nested prefixes', () => {
    window.localStorage.setItem('authorization', 'token');

    expect(getNormalizedRoutePath('/ticket/42', options)).toBe('/ticket/42');
    expect(getNormalizedRoutePath('/dashboard/ticket/42', options)).toBe('/ticket/42');
  });
});

describe('stripBasePath', () => {
  it('strips the admin basename down to app-relative route paths', () => {
    expect(stripBasePath('/secure-admin/dashboard', '/secure-admin')).toBe('/dashboard');
    expect(stripBasePath('/secure-admin', '/secure-admin')).toBe('/');
    expect(stripBasePath('/secure-admin/ticket/42', '/secure-admin')).toBe('/ticket/42');
  });

  it('returns non-matching and root-based paths unchanged', () => {
    expect(stripBasePath('/dashboard', '/')).toBe('/dashboard');
    expect(stripBasePath('/other/dashboard', '/secure-admin')).toBe('/other/dashboard');
    // Segment boundary: /secure-admin2 is NOT under /secure-admin.
    expect(stripBasePath('/secure-admin2/dashboard', '/secure-admin')).toBe(
      '/secure-admin2/dashboard',
    );
  });
});

// docs/api-dialect.md §10.3: the boot translator turns a legacy `#/x?y` entry
// into a history URL before router creation, honoring the injected
// `legacy_hash_redirect_enable` toggle.
describe('legacy hash redirect boot translator', () => {
  beforeEach(() => {
    window.history.replaceState(null, '', '/');
  });

  it('maps legacy hashes to history URLs, resolving the admin base', () => {
    expect(legacyHashHistoryUrl('#/order/T1?from=mail')).toBe('/order/T1?from=mail');
    expect(legacyHashHistoryUrl('#/dashboard', '/secure-admin')).toBe('/secure-admin/dashboard');
    expect(legacyHashHistoryUrl('#/', '/secure-admin')).toBe('/secure-admin/');
  });

  it('leaves invalid or foreign hashes alone', () => {
    expect(legacyHashHistoryUrl('')).toBeNull();
    expect(legacyHashHistoryUrl('#cashier')).toBeNull();
    expect(legacyHashHistoryUrl('#')).toBeNull();
  });

  it('replaces the history entry only when enabled and a legacy hash is present', () => {
    window.history.replaceState(null, '', '/#/order/T1?from=mail');
    expect(applyLegacyHashRedirect({ enabled: true })).toBe(true);
    expect(`${window.location.pathname}${window.location.search}`).toBe('/order/T1?from=mail');
    expect(window.location.hash).toBe('');
  });

  it('resolves against the admin basename', () => {
    window.history.replaceState(null, '', '/secure-admin#/config/system');
    expect(applyLegacyHashRedirect({ enabled: true, basename: '/secure-admin' })).toBe(true);
    expect(window.location.pathname).toBe('/secure-admin/config/system');
  });

  it('ignores the hash entirely when the toggle is OFF', () => {
    window.history.replaceState(null, '', '/#/dashboard');
    expect(applyLegacyHashRedirect({ enabled: false })).toBe(false);
    expect(window.location.hash).toBe('#/dashboard');
  });

  it('does nothing on plain history URLs', () => {
    window.history.replaceState(null, '', '/dashboard');
    expect(applyLegacyHashRedirect({ enabled: true })).toBe(false);
    expect(window.location.pathname).toBe('/dashboard');
  });
});
