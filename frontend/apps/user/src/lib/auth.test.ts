import { QueryClient } from '@tanstack/react-query';
import { afterEach, describe, expect, it } from 'vitest';
import {
  clearSessionCaches,
  getAuthData,
  logout,
  registerSessionCacheClearer,
  setAuthData,
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
});
