import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getAuthData,
  logout,
  registerSessionCacheClearer,
  setAuthData,
  setupAuthSync,
  subscribeAuth,
} from './auth';

describe('admin auth storage', () => {
  afterEach(() => {
    registerSessionCacheClearer(() => undefined);
    localStorage.clear();
  });

  it('uses its own admin-only localStorage key, independent of the user app', () => {
    setAuthData('admin-token');

    expect(localStorage.getItem('v2board.admin_auth_data')).toBe('admin-token');
    expect(localStorage.getItem('authorization')).toBeNull();
    expect(getAuthData()).toBe('admin-token');
  });

  it('notifies subscribers and removes the token on logout', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeAuth(listener);

    setAuthData('jwt');
    logout();
    unsubscribe();

    expect(listener).toHaveBeenNthCalledWith(1, 'jwt');
    expect(listener).toHaveBeenNthCalledWith(2, null);
    expect(localStorage.getItem('v2board.admin_auth_data')).toBeNull();
  });

  it('clears all admin server state when the token identity changes', () => {
    const clear = vi.fn();
    registerSessionCacheClearer(clear);

    setAuthData('first-admin');
    clear.mockClear();
    setAuthData('second-admin');

    expect(clear).toHaveBeenCalledOnce();
    expect(getAuthData()).toBe('second-admin');
  });

  it('synchronizes auth teardown from another browser tab', () => {
    const clear = vi.fn();
    const listener = vi.fn();
    registerSessionCacheClearer(clear);
    const unsubscribe = subscribeAuth(listener);
    setupAuthSync();

    window.dispatchEvent(
      new StorageEvent('storage', { key: 'v2board.admin_auth_data', newValue: null }),
    );

    expect(clear).toHaveBeenCalledOnce();
    expect(listener).toHaveBeenCalledWith(null);
    unsubscribe();
  });
});
