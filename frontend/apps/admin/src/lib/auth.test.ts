import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getAuthData,
  logout,
  registerSessionCacheClearer,
  setAuthData,
  setupAuthSync,
  subscribeAuth,
} from './auth';

describe('admin legacy auth storage', () => {
  afterEach(() => {
    registerSessionCacheClearer(() => undefined);
    localStorage.clear();
  });

  it('uses the original authorization localStorage key shared by the legacy bundles', () => {
    setAuthData('legacy-token');

    expect(localStorage.getItem('authorization')).toBe('legacy-token');
    expect(localStorage.getItem('v2board.admin_auth_data')).toBeNull();
    expect(getAuthData()).toBe('legacy-token');
  });

  it('notifies subscribers and removes the legacy token on logout', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeAuth(listener);

    setAuthData('jwt');
    logout();
    unsubscribe();

    expect(listener).toHaveBeenNthCalledWith(1, 'jwt');
    expect(listener).toHaveBeenNthCalledWith(2, null);
    expect(localStorage.getItem('authorization')).toBeNull();
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

    window.dispatchEvent(new StorageEvent('storage', { key: 'authorization', newValue: null }));

    expect(clear).toHaveBeenCalledOnce();
    expect(listener).toHaveBeenCalledWith(null);
    unsubscribe();
  });
});
