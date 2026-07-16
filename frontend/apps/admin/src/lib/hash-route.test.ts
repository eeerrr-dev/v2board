import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { matchPath } from 'react-router';
import { getNormalizedHashPath, type HashRouteOptions } from '@v2board/config';

const options: HashRouteOptions = {
  matchRoute: (route, path, end) => matchPath({ path: route, end }, path),
  authenticatedFallback: '/dashboard',
  authenticatedPublicFallbackRoutes: [],
  guestFallback: '/login',
  nestedPrefixes: ['/dashboard', '/ticket/:ticket_id'],
  publicRoutes: ['/', '/login', '/register'],
  routes: ['/', '/login', '/register', '/dashboard', '/ticket', '/ticket/:ticket_id'],
};

describe('hash-route contract adapter', () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.history.replaceState(null, '', '/');
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('preserves public routes and query strings for guests', () => {
    expect(getNormalizedHashPath('/register?invite=abc', options)).toBe('/register?invite=abc');
  });

  it('redirects protected and unknown routes according to authentication', () => {
    expect(getNormalizedHashPath('/dashboard', options)).toBe('/login');
    expect(getNormalizedHashPath('/unknown', options)).toBe('/login');

    window.localStorage.setItem('authorization', 'token');
    expect(getNormalizedHashPath('/dashboard', options)).toBe('/dashboard');
    expect(getNormalizedHashPath('/unknown', options)).toBe('/dashboard');
    expect(getNormalizedHashPath('/login', options)).toBe('/login');
  });

  it('recognizes dynamic routes and recovers duplicated nested prefixes', () => {
    window.localStorage.setItem('authorization', 'token');

    expect(getNormalizedHashPath('/ticket/42', options)).toBe('/ticket/42');
    expect(getNormalizedHashPath('/dashboard/ticket/42', options)).toBe('/ticket/42');
  });
});
