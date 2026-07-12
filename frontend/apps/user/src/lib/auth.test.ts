import { QueryClient } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  clearSessionCaches,
  getAuthData,
  logout,
  normalizeLoginRedirectTarget,
  registerSessionCacheClearer,
  setAuthData,
  setupAuthSync,
  subscribeAuth,
} from './auth';
import { userKeys } from './queries';

describe('auth session teardown', () => {
  afterEach(() => {
    registerSessionCacheClearer(() => undefined);
    setAuthData(null);
  });

  it('logout clears the auth key and the registered session cache', () => {
    const queryClient = new QueryClient();
    registerSessionCacheClearer(() => queryClient.clear());
    setAuthData('token-abc');
    queryClient.setQueryData(userKeys.subscribe, {
      subscribe_url: 'https://sub.example/credential',
    });

    logout();

    // The next session on this tab must not be able to read the previous
    // account's cached server state (subscribe_url is a credential).
    expect(getAuthData()).toBeNull();
    expect(queryClient.getQueryData(userKeys.subscribe)).toBeUndefined();
  });

  it('clearSessionCaches invokes the latest registered clearer', () => {
    const calls: string[] = [];
    registerSessionCacheClearer(() => calls.push('first'));
    registerSessionCacheClearer(() => calls.push('second'));

    clearSessionCaches();

    expect(calls).toEqual(['second']);
  });

  it('clears session caches when one authenticated identity replaces another', () => {
    const queryClient = new QueryClient();
    registerSessionCacheClearer(() => queryClient.clear());
    setAuthData('first-user');
    queryClient.setQueryData(userKeys.info, { email: 'first@example.com' });

    setAuthData('second-user');

    expect(getAuthData()).toBe('second-user');
    expect(queryClient.getQueryData(userKeys.info)).toBeUndefined();
  });

  it('synchronizes identity changes from another browser tab', () => {
    const clear = vi.fn();
    const listener = vi.fn();
    registerSessionCacheClearer(clear);
    const unsubscribe = subscribeAuth(listener);
    setupAuthSync();
    clear.mockClear();

    window.dispatchEvent(
      new StorageEvent('storage', {
        key: 'authorization',
        newValue: 'other-tab-token',
      }),
    );

    expect(clear).toHaveBeenCalledOnce();
    expect(listener).toHaveBeenCalledWith('other-tab-token');
    unsubscribe();
  });
});

describe('post-login redirect normalization', () => {
  it('keeps internal targets and repairs bare relative paths', () => {
    expect(normalizeLoginRedirectTarget(null)).toBe('/dashboard');
    expect(normalizeLoginRedirectTarget('/order?from=login')).toBe('/order?from=login');
    expect(normalizeLoginRedirectTarget('order')).toBe('/order');
  });

  it('rejects protocol-relative targets after browser-equivalent normalization', () => {
    expect(normalizeLoginRedirectTarget('//evil.example/path')).toBe('/dashboard');
    expect(normalizeLoginRedirectTarget('/\\evil.example/path')).toBe('/dashboard');
    expect(normalizeLoginRedirectTarget('/\t/evil.example/path')).toBe('/dashboard');
  });
});
