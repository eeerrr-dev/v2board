import { describe, expect, it, vi } from 'vitest';
import AxiosMockAdapter from 'axios-mock-adapter';
import { ApiError, createApiClient } from './client';
import { login, token2Login } from './endpoints/passport';

describe('createApiClient', () => {
  it('unwraps the data envelope', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onPost('/passport/auth/login')
      .reply(200, { data: { token: 't', is_admin: 0, auth_data: 'jwt' } });
    const result = await login(client, { email: 'a@b.c', password: 'x' });
    expect(result).toEqual({ token: 't', is_admin: 0, auth_data: 'jwt' });
  });

  it('uses the legacy form body and request headers', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      getAuthData: () => 'auth',
      getLocale: () => 'zh-CN',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onPost('/passport/auth/login')
      .reply(200, { data: { token: 't', is_admin: 0, auth_data: 'jwt' } });
    await login(client, { email: 'a@b.c', password: 'x' });
    const request = mock.history.post[0]!;
    expect(request.data).toBe('email=a%40b.c&password=x');
    expect(request.headers?.authorization).toBe('auth');
    expect(request.headers?.['Content-Language']).toBe('zh-CN');
    expect(request.withCredentials).toBe(true);
  });

  it('sends empty POST requests as legacy form requests', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/newPeriod').reply(200, { data: true });
    await client.request({ url: '/user/newPeriod', method: 'POST' });
    const request = mock.history.post[0]!;
    expect(request.data).toBe('');
    expect(request.headers?.['Content-Type']).toBe('application/x-www-form-urlencoded');
  });

  it('uses the legacy GET request for token login', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onGet('/passport/auth/token2Login?verify=abc')
      .reply(200, { data: { token: 't', is_admin: 0, auth_data: 'jwt' } });
    await token2Login(client, { verify: 'abc' });
    expect(mock.history.get[0]?.url).toBe('/passport/auth/token2Login?verify=abc');
  });

  it('maps 403 into ApiError and fires onUnauthorized', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(403, { message: 'auth required' });
    await expect(client.request({ url: '/user/info', method: 'GET' })).rejects.toBeInstanceOf(
      ApiError,
    );
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });

  it('maps other non-2xx responses into ApiError without unauthorized handling', async () => {
    const onUnauthorized = vi.fn();
    const onError = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized, onError });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(401, { message: 'auth required' });
    await expect(client.request({ url: '/user/info', method: 'GET' })).rejects.toBeInstanceOf(
      ApiError,
    );
    expect(onUnauthorized).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledOnce();
  });

  it('uses the first legacy validation error as the ApiError message', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/passport/auth/login').reply(422, {
      message: 'validation failed',
      errors: { email: ['Email cannot be empty'] },
    });
    await expect(login(client, { email: '', password: '' })).rejects.toMatchObject({
      message: 'Email cannot be empty',
    });
  });

  it('prefixes admin paths when securePath is provided', () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'abc' });
    expect(client.resolveAdminPath('/plan/fetch')).toBe('/abc/plan/fetch');
  });
});
