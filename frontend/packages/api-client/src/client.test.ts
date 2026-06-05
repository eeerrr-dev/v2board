import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import AxiosMockAdapter from 'axios-mock-adapter';
import { ApiError, createApiClient } from './client';
import {
  assignOrder,
  dumpUsersCsv,
  fetchCoupons as fetchAdminCoupons,
  fetchGiftcards as fetchAdminGiftcards,
  fetchOrders as fetchAdminOrders,
  fetchTickets as fetchAdminTickets,
  fetchUsers,
  getUserInfoById,
  fetchNotices,
  fetchConfig,
  generateCoupon,
  generateGiftcard,
  generateUser,
  fetchPlans,
  savePlan,
  sendMailToUsers,
  setTelegramWebhook,
  sortServerNodes,
  statUser,
  testSendMail,
  updatePlan,
  updateServer,
} from './endpoints/admin';
import { login, token2Login } from './endpoints/passport';
import * as passportEndpoints from './endpoints/passport';
import * as userEndpoints from './endpoints/user';

function textBuffer(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer as ArrayBuffer;
}

describe('createApiClient', () => {
  it('does not expose passport endpoints absent from the original user bundle', () => {
    const endpoints = passportEndpoints as unknown as Record<string, unknown>;

    expect(endpoints.pv).toBeUndefined();
    expect(endpoints.getQuickLoginUrl).toBeUndefined();
  });

  it('does not expose user session endpoints absent from the original user bundle', () => {
    const endpoints = userEndpoints as unknown as Record<string, unknown>;

    expect(endpoints.getActiveSession).toBeUndefined();
    expect(endpoints.removeActiveSession).toBeUndefined();
  });

  it('does not expose a user notice detail endpoint absent from the original user bundle', () => {
    const endpoints = userEndpoints as unknown as Record<string, unknown>;

    expect(endpoints.noticeDetail).toBeUndefined();
  });

  it('keeps user notice fetch as the bundled notice model data-only state', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/notice/fetch').reply(200, { data: [], total: 99 });

    await expect(userEndpoints.fetchNotices(client)).resolves.toEqual({ data: [] });
  });

  it('does not synthesize a user notice total absent from the bundled notice model', () => {
    const source = readFileSync(new URL('./endpoints/user.ts', import.meta.url), 'utf8');

    expect(source).toContain('return { data: env.data };');
    expect(source).not.toContain('total: env.total ?? 0');
  });

  it('exposes the original tutorial fetch endpoints and parses detail steps like the old model', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/tutorial/fetch').reply(200, {
      data: { tutorials: [{ id: 1, title: 'Windows' }], safe_area_var: { top: 0 } },
    });
    mock.onGet('/user/tutorial/fetch?id=1').reply(200, {
      data: { id: 1, title: 'Windows', steps: '[{\"title\":\"Install\"}]' },
    });

    await expect(userEndpoints.fetchTutorials(client)).resolves.toEqual({
      tutorials: [{ id: 1, title: 'Windows' }],
      safe_area_var: { top: 0 },
    });
    await expect(userEndpoints.tutorialDetail(client, 1)).resolves.toEqual({
      id: 1,
      title: 'Windows',
      steps: [{ title: 'Install' }],
    });
    expect(mock.history.get[0]?.url).toBe('/user/tutorial/fetch');
    expect(mock.history.get[1]?.url).toBe('/user/tutorial/fetch?id=1');
  });

  it('unwraps the data envelope', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onPost('/passport/auth/login')
      .reply(200, { data: { token: 't', is_admin: 0, auth_data: 'jwt' } });
    const result = await login(client, { email: 'a@b.c', password: 'x' });
    expect(result).toEqual({ token: 't', is_admin: 0, auth_data: 'jwt' });
  });

  it('treats legacy 200 HTTP responses with non-200 business code as failures', async () => {
    const onError = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onError });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/passport/auth/login').reply(200, { code: 500, message: 'invalid login' });
    await expect(login(client, { email: 'a@b.c', password: 'x' })).rejects.toMatchObject({
      status: 500,
      message: 'invalid login',
    });
    expect(onError).not.toHaveBeenCalled();
  });

  it('adds the legacy HTTP 200 code to successful envelopes when the body omits it', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/notice/fetch').reply(200, { data: [], total: 0 });

    await expect(
      client.requestEnvelope<unknown[]>({ url: '/user/notice/fetch', method: 'GET' }),
    ).resolves.toEqual({ code: 200, data: [], total: 0 });
  });

  it('keeps checkout response type as the direct envelope field', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/order/checkout').reply(200, { data: 'https://pay.example.test' });

    const result = await userEndpoints.checkoutOrder(client, { trade_no: 'T1', method: 1 });

    expect((result as { type?: unknown }).type).toBeUndefined();
  });

  it('keeps invite detail totals as the direct envelope field', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/invite/details?current=1&page_size=10').reply(200, { data: [] });

    const result = await userEndpoints.inviteDetails(client, 1, 10);

    expect(result.total).toBeUndefined();
  });

  it('submits legacy user transfer amounts in cents from the API layer', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/transfer').reply(200, { data: true });

    await userEndpoints.transfer(client, '12.34');

    expect(mock.history.post[0]?.data).toBe('transfer_amount=1234');
  });

  it('uses the legacy user transfer multiplication shape in the API layer', () => {
    const source = readFileSync(new URL('./endpoints/user.ts', import.meta.url), 'utf8');

    expect(source).toContain('transfer_amount: 100 * (transferAmount as number)');
    expect(source).not.toContain('Number(transferAmount)');
  });

  it('keeps redeem gift card type and value on the legacy response envelope', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/redeemgiftcard').reply(200, { data: true, type: 1, value: 1234 });

    await expect(userEndpoints.redeemGiftCard(client, 'CARD-123')).resolves.toEqual({
      type: 1,
      value: 1234,
    });
    expect(mock.history.post[0]?.data).toBe('giftcard=CARD-123');
  });

  it('treats HTTP 201 as a legacy request failure', async () => {
    const onError = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onError });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(201, { message: 'created' });

    await expect(client.request({ url: '/test', method: 'POST' })).rejects.toMatchObject({
      status: 201,
      message: 'created',
    });
    expect(onError).toHaveBeenCalledOnce();
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

  it('omits nullish values from legacy form bodies', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(200, { data: true });
    await client.request({
      url: '/test',
      method: 'POST',
      data: { keep: 'yes', skipNull: null, skipUndefined: undefined },
    });
    expect(mock.history.post[0]?.data).toBe('keep=yes');
  });

  it('serializes admin legacy null form values as empty strings', async () => {
    const client = createApiClient({ baseURL: '/api/v1', nullFormValue: 'empty' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(200, { data: true });
    await client.request({
      url: '/test',
      method: 'POST',
      data: { clear: null, filter: [{ key: 'plan_id', condition: '=', value: null }] },
    });

    expect(mock.history.post[0]?.data).toBe(
      'clear=&filter[0][key]=plan_id&filter[0][condition]=%3D&filter[0][value]=',
    );
  });

  it('uses legacy recursive bracket encoding for arrays and objects', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(200, { data: true });
    await client.request({
      url: '/test',
      method: 'POST',
      data: { list: ['a', 'b'], nested: { key: 'value' } },
    });
    expect(mock.history.post[0]?.data).toBe('list[0]=a&list[1]=b&nested[key]=value');
  });

  it('uses the legacy for-in enumeration rules while serializing forms', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const payload = Object.create({ inherited: 'legacy' }) as Record<string, unknown>;
    payload.own = 'value';

    mock.onPost('/test').reply(200, { data: true });
    await client.request({ url: '/test', method: 'POST', data: payload });

    expect(mock.history.post[0]?.data).toBe('own=value&inherited=legacy');
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

  it('maps legacy envelope code 403 into ApiError and fires onUnauthorized', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(200, { code: 403, message: 'auth required' });

    await expect(client.request({ url: '/user/info', method: 'GET' })).rejects.toMatchObject({
      status: 403,
      message: 'auth required',
    });
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

  it('keeps legacy admin page totals as direct envelope fields', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/user/fetch').reply(200, { data: [] });
    mock.onGet('/admin-path/order/fetch').reply(200, { data: [] });
    mock.onGet('/admin-path/ticket/fetch').reply(200, { data: [] });
    mock.onGet('/admin-path/coupon/fetch').reply(200, { data: [] });
    mock.onGet('/admin-path/giftcard/fetch').reply(200, { data: [] });
    mock.onGet('/admin-path/stat/getStatUser?user_id=1').reply(200, { data: [] });

    await expect(fetchUsers(client)).resolves.toEqual({ data: [], total: undefined });
    await expect(fetchAdminOrders(client)).resolves.toEqual({ data: [], total: undefined });
    await expect(fetchAdminTickets(client)).resolves.toEqual({ data: [], total: undefined });
    await expect(fetchAdminCoupons(client)).resolves.toEqual({ data: [], total: undefined });
    await expect(fetchAdminGiftcards(client)).resolves.toEqual({ data: [], total: undefined });
    await expect(statUser(client, { user_id: 1 })).resolves.toEqual({ data: [], total: undefined });
  });

  it('normalizes fetched coupon and giftcard amount values to legacy model units', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const couponRows = [
      { id: 1, type: 1, value: 1234 },
      { id: 2, type: 2, value: 30 },
    ];
    const giftcardRows = [
      { id: 1, type: 1, value: 5678 },
      { id: 2, type: 3, value: 100 },
    ];
    mock.onGet('/admin-path/coupon/fetch').reply(200, { data: couponRows, total: 2 });
    mock.onGet('/admin-path/giftcard/fetch').reply(200, { data: giftcardRows, total: 2 });

    await expect(fetchAdminCoupons(client)).resolves.toEqual({
      data: [
        { id: 1, type: 1, value: 12.34 },
        { id: 2, type: 2, value: 30 },
      ],
      total: 2,
    });
    await expect(fetchAdminGiftcards(client)).resolves.toEqual({
      data: [
        { id: 1, type: 1, value: 56.78 },
        { id: 2, type: 3, value: 100 },
      ],
      total: 2,
    });
  });

  it('requests legacy admin user traffic records with pagination', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onAny().reply((config) => {
      expect(config.method).toBe('get');
      expect(config.url).toBe(
        '/admin-path/stat/getStatUser?user_id=1&current=2&pageSize=10&total=1',
      );
      expect(config.params).toBeUndefined();
      return [200, {
        data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
        total: 1,
      }];
    });

    await expect(
      statUser(client, { user_id: 1, current: 2, pageSize: 10, total: 1 }),
    ).resolves.toEqual({
      data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
      total: 1,
    });
    expect(mock.history.get[0]?.url).toBe(
      '/admin-path/stat/getStatUser?user_id=1&current=2&pageSize=10&total=1',
    );
  });

  it('submits legacy admin assigned-order amounts in cents', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/order/assign').reply(200, { data: 'TRADE123' });

    await assignOrder(client, {
      email: 'user@example.com',
      plan_id: 1,
      period: 'month_price',
      total_amount: '12.34',
    });

    expect(mock.history.post[0]?.data).toBe(
      'email=user%40example.com&plan_id=1&period=month_price&total_amount=1234',
    );
  });

  it('uses the legacy direct multiplication shape for assigned-order amounts', () => {
    const source = readFileSync(new URL('./endpoints/admin.ts', import.meta.url), 'utf8');

    expect(source).toContain('total_amount: 100 * (data.total_amount as number),');
    expect(source).not.toContain('total_amount: Number(data.total_amount) * 100');
  });

  it('submits admin plan updates with the original dynamic key payload', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/plan/update').reply(200, { data: true });

    await updatePlan(client, 7, 'renew', 0);

    expect(mock.history.post[0]?.data).toBe('id=7&renew=0');
  });

  it('uses the legacy key/value shape for admin plan update requests', () => {
    const source = readFileSync(new URL('./endpoints/admin.ts', import.meta.url), 'utf8');

    expect(source).toContain("key: 'show' | 'renew',");
    expect(source).toContain("value: 0 | 1,");
    expect(source).toContain("adminPost<true>(client, '/plan/update', { id, [key]: value })");
    expect(source).not.toContain('show?: 0 | 1, renew?: 0 | 1');
    expect(source).not.toContain('{ id, show, renew }');
  });

  it('normalizes legacy admin user rows exactly like the packaged admin model', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const rawUser = {
      id: 1,
      email: 'user@example.com',
      password: 'secret',
      balance: 1234,
      commission_balance: 5678,
      transfer_enable: 107374182400,
      device_limit: 3,
      u: 1073741824,
      d: 2147483648,
      total_used: 3221225472,
      alive_ip: 2,
      ips: '127.0.0.1',
      plan_id: 1,
      plan_name: '基础套餐',
      group_id: 1,
      expired_at: 1893456000,
      uuid: 'uuid',
      token: 'token',
      subscribe_url: 'https://example.com/sub',
      banned: 0,
      is_admin: 0,
      is_staff: 0,
      invite_user_id: null,
      invite_user: { email: 'invite@example.com' },
      discount: null,
      commission_rate: null,
      telegram_id: null,
      last_login_at: 1700000000,
      created_at: 1700000000,
      updated_at: 1700000000,
    };
    mock.onGet('/admin-path/user/fetch?current=1').reply(200, {
      data: [rawUser],
      total: 1,
    });
    mock.onGet('/admin-path/user/getUserInfoById?id=1').reply(200, { data: rawUser });

    await expect(fetchUsers(client, { current: 1 })).resolves.toMatchObject({
      data: [
        {
          password: '',
          balance: '12.34',
          commission_balance: '56.78',
          transfer_enable: '100.00',
          u: '1.00',
          d: '2.00',
          total_used: '3.00',
        },
      ],
      total: 1,
    });
    await expect(fetchUsers(client, { current: 1 })).resolves.toMatchObject({
      data: [expect.not.objectContaining({ invite_user_email: 'invite@example.com' })],
    });
    await expect(getUserInfoById(client, 1)).resolves.toMatchObject({
      password: '',
      balance: '12.34',
      commission_balance: '56.78',
      transfer_enable: '100.00',
      u: '1.00',
      d: '2.00',
      total_used: 3221225472,
      invite_user_email: 'invite@example.com',
    });
  });

  it('submits legacy server-manage sort payloads as JSON', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const payload = { shadowsocks: { 1: 0, 3: 2 }, vmess: { 9: 1 } };
    mock.onPost('/admin-path/server/manage/sort').reply(200, { data: true });

    await sortServerNodes(client, payload);

    expect(mock.history.post[0]?.data).toBe(JSON.stringify(payload));
    expect(mock.history.post[0]?.headers?.['Content-Type']).toBe('application/json');
  });

  it('submits admin server updates with the original dynamic key payload', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/server/vmess/update').reply(200, { data: true });

    await updateServer(client, 'vmess', 8, 'show', 0);

    expect(mock.history.post[0]?.data).toBe('id=8&show=0');
  });

  it('uses the legacy key/value shape for admin server update requests', () => {
    const source = readFileSync(new URL('./endpoints/admin.ts', import.meta.url), 'utf8');

    expect(source).toContain("key: 'show',");
    expect(source).toContain("value: 0 | 1,");
    expect(source).toContain('adminPost<true>(client, `/server/${type}/update`, { id, [key]: value })');
    expect(source).not.toContain('show: 0 | 1,\n) => adminPost<true>(client, `/server/${type}/update`, { id, show })');
  });

  it('submits legacy admin plan prices in cents from the API layer', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
      nullFormValue: 'empty',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/plan/save').reply(200, { data: true });

    await savePlan(client, {
      id: 1,
      name: '基础套餐',
      month_price: '12.34',
      quarter_price: 0,
      half_year_price: null,
      year_price: '',
      onetime_price: 300,
      force_update: true,
    });

    const body = new URLSearchParams(String(mock.history.post[0]?.data));
    expect(body.get('id')).toBe('1');
    expect(body.get('name')).toBe('基础套餐');
    expect(body.get('month_price')).toBe('1234');
    expect(body.get('quarter_price')).toBe('0');
    expect(body.get('year_price')).toBe('0');
    expect(body.get('onetime_price')).toBe('30000');
    expect(body.get('force_update')).toBe('true');
    expect(body.get('half_year_price')).toBe('');
  });

  it('normalizes legacy admin plan prices with the original null-only check', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/plan/fetch').reply(200, {
      data: [
        {
          id: 1,
          name: '基础套餐',
          month_price: 1234,
          quarter_price: null,
          half_year_price: '',
          year_price: undefined,
        },
      ],
    });

    const result = await fetchPlans(client);

    expect(result[0]?.month_price).toBe(12.34);
    expect(result[0]?.quarter_price).toBeNull();
    expect(result[0]?.half_year_price).toBe(0);
    expect(Number.isNaN(result[0]?.year_price as number)).toBe(true);
  });

  it('normalizes legacy admin config comma-list strings after fetch', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/config/fetch').reply(200, {
      data: {
        deposit: { deposit_bounus: '50:18,100:38' },
        invite: { commission_withdraw_method: '支付宝,USDT' },
        site: { email_whitelist_suffix: 'qq.com,gmail.com' },
        safe: { email_whitelist_suffix: 'safe.example' },
      },
    });

    const result = await fetchConfig(client);

    expect(result.deposit.deposit_bounus).toEqual(['50:18', '100:38']);
    expect(result.deposit_bounus).toEqual(['50:18', '100:38']);
    expect(result.invite.commission_withdraw_method).toEqual(['支付宝', 'USDT']);
    expect(result.commission_withdraw_method).toEqual(['支付宝', 'USDT']);
    expect((result.site as unknown as Record<string, unknown>).email_whitelist_suffix).toEqual([
      'qq.com',
      'gmail.com',
    ]);
    expect(result.email_whitelist_suffix).toBe('safe.example');
  });

  it('keeps legacy admin notice fetch as a plain unpaginated array response', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/notice/fetch').reply(200, {
      data: [
        {
          id: 1,
          title: '维护通知',
          content: 'content',
          img_url: null,
          tags: ['system'],
          show: 1,
          created_at: 1700000000,
          updated_at: 1700000000,
        },
      ],
    });

    await expect(fetchNotices(client, { current: 1, pageSize: 10 })).resolves.toMatchObject({
      data: [{ id: 1, title: '维护通知' }],
      total: undefined,
    });
    expect(mock.history.get[0]?.url).toBe('/admin-path/notice/fetch');
  });

  it('sets the telegram webhook with the original empty token payload', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/config/setTelegramWebhook').reply(200, { data: true });

    await setTelegramWebhook(client);

    expect(mock.history.post[0]?.data).toBe('');
  });

  it('sends the original empty test-mail request and preserves its log envelope', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/config/testSendMail').reply(200, {
      code: 200,
      data: true,
      log: {
        email: 'user@example.com',
        config: { host: 'smtp.example.com', port: 465, encryption: 'ssl', username: 'mailer' },
      },
    });

    await expect(testSendMail(client)).resolves.toMatchObject({
      code: 200,
      data: true,
      log: { email: 'user@example.com' },
    });
    expect(mock.history.post[0]?.data).toBe('');
  });

  it('preserves legacy generated user CSV buffers', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const csvBuffer = textBuffer('user-csv');
    mock.onPost('/admin-path/user/generate').reply(200, csvBuffer, {
      'content-type': 'text/csv',
    });

    await expect(
      generateUser(client, {
        email_suffix: 'example.com',
        generate_count: '2',
      }),
    ).resolves.toMatchObject({ buffer: csvBuffer });
    expect(mock.history.post[0]?.data).toBe('email_suffix=example.com&generate_count=2');
    expect(mock.history.post[0]?.responseType).toBe('arraybuffer');
  });

  it('keeps JSON responses usable on legacy admin CSV-capable endpoints', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/user/generate').reply(200, textBuffer(JSON.stringify({ data: true })), {
      'content-type': 'application/json',
    });

    await expect(generateUser(client, { email_suffix: 'example.com' })).resolves.toMatchObject({
      code: 200,
      data: true,
    });
  });

  it('uses the legacy exact application/json check for CSV-capable admin responses', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const jsonBuffer = textBuffer(JSON.stringify({ data: true }));
    mock.onPost('/admin-path/user/generate').reply(200, jsonBuffer, {
      'content-type': 'application/json; charset=utf-8',
    });

    await expect(generateUser(client, { email_suffix: 'example.com' })).resolves.toMatchObject({
      buffer: jsonBuffer,
    });
  });

  it('preserves legacy dumped user CSV buffers', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const csvBuffer = textBuffer('users-csv');
    mock.onPost('/admin-path/user/dumpCSV').reply(200, csvBuffer, {
      'content-type': 'text/csv',
    });

    await expect(
      dumpUsersCsv(client, [{ key: 'email', condition: '模糊', value: 'user@example.com' }]),
    ).resolves.toMatchObject({ buffer: csvBuffer });
    expect(mock.history.post[0]?.data).toBe(
      'filter[0][key]=email&filter[0][condition]=%E6%A8%A1%E7%B3%8A&filter[0][value]=user%40example.com',
    );
  });

  it('allows legacy admin user emails to omit empty subject and content fields', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/user/sendMail').reply(200, { data: true });

    await sendMailToUsers(client, {
      filter: [{ key: 'email', condition: '模糊', value: 'user@example.com' }],
    });

    expect(mock.history.post[0]?.data).toBe(
      'filter[0][key]=email&filter[0][condition]=%E6%A8%A1%E7%B3%8A&filter[0][value]=user%40example.com',
    );
  });

  it('preserves legacy generated coupon and giftcard CSV buffers', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const couponBuffer = textBuffer('coupon-code');
    const giftcardBuffer = textBuffer('giftcard-code');
    mock.onPost('/admin-path/coupon/generate').reply(200, couponBuffer, {
      'content-type': 'text/csv',
    });
    mock.onPost('/admin-path/giftcard/generate').reply(200, giftcardBuffer, {
      'content-type': 'text/csv',
    });

    await expect(generateCoupon(client, { generate_count: '2' })).resolves.toMatchObject({
      buffer: couponBuffer,
    });
    await expect(generateGiftcard(client, { generate_count: '2' })).resolves.toMatchObject({
      buffer: giftcardBuffer,
    });
  });
});
