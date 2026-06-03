import { afterEach, describe, expect, it, vi } from 'vitest';
import { getAuthData, logout, setAuthData, subscribeAuth } from './auth';

describe('admin legacy auth storage', () => {
  afterEach(() => {
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
});
