import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import AxiosMockAdapter from 'axios-mock-adapter';
import { z } from 'zod';
import { ApiContractError, ApiError, createApiClient } from './client';
import { ApiProblemError } from './dialect';
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
  fetchPayments as fetchAdminPayments,
  fetchServerNodes,
  fetchConfig,
  fetchSystemLogs,
  generateCoupon,
  generateGiftcard,
  generateUser,
  fetchPlans,
  knowledgeDetail,
  saveConfig,
  savePlan,
  savePayment,
  saveServer,
  sendMailToUsers,
  setTelegramWebhook,
  showServer,
  sortServerNodes,
  statUser,
  testSendMail,
  updateCoupon,
  updatePlan,
  updateUser,
} from './endpoints/admin';
import { config as fetchGuestConfig } from './endpoints/guest';
import { login, tokenLogin } from './endpoints/passport';
import * as passportEndpoints from './endpoints/passport';
import * as userEndpoints from './endpoints/user';

function textBuffer(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer as ArrayBuffer;
}

function makeCoupon(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    code: 'CODE',
    name: 'Coupon',
    type: 1,
    value: 100,
    show: true,
    limit_use: null,
    limit_use_with_user: null,
    limit_plan_ids: null,
    limit_period: null,
    started_at: '2023-11-14T22:13:20Z',
    ended_at: '2023-11-14T23:13:20Z',
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
    ...overrides,
  };
}

function makeGiftcard(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    name: 'Gift card',
    code: 'GIFT',
    type: 1,
    value: 100,
    plan_id: null,
    limit_use: null,
    used_user_ids: [],
    started_at: '2023-11-14T22:13:20Z',
    ended_at: '2023-11-14T23:13:20Z',
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
    ...overrides,
  };
}

function makePlan(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    group_id: 1,
    transfer_enable: 100,
    device_limit: null,
    speed_limit: null,
    reset_traffic_method: null,
    name: 'Plan',
    show: true,
    sort: null,
    renew: true,
    content: null,
    month_price: 1000,
    quarter_price: null,
    half_year_price: null,
    year_price: null,
    two_year_price: null,
    three_year_price: null,
    onetime_price: null,
    reset_price: null,
    capacity_limit: null,
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
    ...overrides,
  };
}

// GET `/{secure_path}/config` (docs/api-dialect.md §6.1, W9): §4.1 native
// JSON types — real booleans, JSON numbers, real arrays, and the recorded
// `commission_withdraw_limit` decimal-string exception.
function makeAdminConfig() {
  return {
    ticket: { ticket_status: 0 },
    deposit: { deposit_bounus: ['50:18', '100:38'] },
    invite: {
      invite_force: true,
      invite_commission: 10,
      invite_gen_limit: 5,
      invite_never_expire: false,
      commission_first_time_enable: true,
      commission_auto_check_enable: true,
      commission_withdraw_limit: '100',
      commission_withdraw_method: ['支付宝', 'USDT'],
      withdraw_close_enable: false,
      commission_distribution_enable: true,
      commission_distribution_l1: 50,
      commission_distribution_l2: 30,
      commission_distribution_l3: 20,
    },
    site: {
      logo: 'https://example.test/logo.png',
      force_https: true,
      stop_register: false,
      app_name: 'V2Board',
      app_description: 'V2Board is best!',
      app_url: 'https://example.test',
      subscribe_url: 'https://sub.example.test',
      subscribe_path: '/api/v1/client/subscribe',
      try_out_plan_id: 1,
      try_out_hour: 24,
      tos_url: 'https://example.test/tos',
      currency: 'CNY',
      currency_symbol: '¥',
      legacy_hash_redirect_enable: true,
    },
    subscribe: {
      plan_change_enable: true,
      reset_traffic_method: 0,
      surplus_enable: true,
      allow_new_period: false,
      new_order_event_id: true,
      renew_order_event_id: false,
      change_order_event_id: true,
      show_info_to_server_enable: true,
      show_subscribe_method: 2,
      show_subscribe_expire: 30,
    },
    frontend: {
      frontend_theme_color: 'default',
      frontend_background_url: null,
      chat_widget_provider: null,
      chat_widget_crisp_website_id: null,
      chat_widget_tawk_property_id: null,
      chat_widget_tawk_widget_id: null,
    },
    server: {
      server_api_url: 'https://node.example.test',
      server_token: 'token',
      server_pull_interval: 60,
      server_push_interval: 60,
      server_node_report_min_traffic: 0,
      server_device_online_min_traffic: 0,
      device_limit_mode: false,
    },
    email: {
      email_template: 'default',
      email_host: 'smtp.example.test',
      email_port: 465,
      email_username: 'mailer',
      email_password: 'password',
      email_encryption: 'ssl',
      email_from_address: 'noreply@example.test',
    },
    telegram: {
      telegram_bot_enable: true,
      telegram_bot_token: 'bot-token',
      telegram_discuss_link: 'https://t.me/example',
    },
    app: {
      windows_version: '1.0.0',
      windows_download_url: 'https://example.test/app.exe',
      macos_version: '1.0.0',
      macos_download_url: 'https://example.test/app.dmg',
      android_version: '1.0.0',
      android_download_url: 'https://example.test/app.apk',
    },
    safe: {
      email_verify: true,
      safe_mode_enable: true,
      admin_mfa_force: false,
      secure_path: 'admin-path',
      email_whitelist_enable: true,
      email_whitelist_suffix: ['qq.com', 'gmail.com'],
      email_gmail_limit_enable: true,
      recaptcha_enable: true,
      recaptcha_key: 'secret',
      recaptcha_site_key: 'site',
      register_limit_by_ip_enable: true,
      register_limit_count: 3,
      register_limit_expire: 60,
      password_limit_enable: true,
      password_limit_count: 5,
      password_limit_expire: 60,
    },
  };
}

describe('createApiClient', () => {
  it('does not expose passport endpoints absent from the original user bundle', () => {
    const endpoints = passportEndpoints as unknown as Record<string, unknown>;

    expect(endpoints.pv).toBeUndefined();
    expect(endpoints.getQuickLoginUrl).toBeUndefined();
  });

  it('fetches the active-session array as bare dialect data', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const sessions = [
      {
        session_id: 'guid-1',
        ip: '1.1.1.1',
        ua: 'Chrome',
        login_at: '2026-01-01T00:00:00Z',
        current: true,
      },
      {
        session_id: 'guid-2',
        ip: '2.2.2.2',
        ua: 'Firefox',
        login_at: '2025-12-31T00:00:00Z',
        current: false,
      },
    ];
    mock.onGet('/user/sessions').reply(200, sessions);

    await expect(userEndpoints.getActiveSession(client)).resolves.toEqual(sessions);
  });

  it('revokes an active session via DELETE /user/sessions/{session_id}', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onDelete('/user/sessions/guid-2').reply(204);

    await expect(userEndpoints.removeActiveSession(client, 'guid-2')).resolves.toBeUndefined();

    expect(mock.history.delete[0]?.url).toBe('/user/sessions/guid-2');
  });

  it('revokes the current session via DELETE /auth/session with an explicitly captured bearer', async () => {
    // Sign-out tears local auth down synchronously right after firing this
    // call, and the request interceptor reads the auth store on a microtask —
    // after that teardown — so the caller passes the captured raw auth_data
    // and the endpoint puts the Bearer scheme on the wire (§4.2).
    const client = createApiClient({ baseURL: '/api/v1', getAuthData: () => null });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onDelete('/auth/session').reply(204);

    await expect(userEndpoints.logout(client, 'captured-raw-token')).resolves.toBeUndefined();

    expect(mock.history.delete[0]?.headers?.authorization).toBe('Bearer captured-raw-token');
  });

  it('probes the session state through GET /auth/session as bare dialect data', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/auth/session').reply(200, { is_login: false });

    await expect(userEndpoints.checkLogin(client)).resolves.toEqual({ is_login: false });
  });

  it('does not expose a user notice detail endpoint absent from the original user bundle', () => {
    const endpoints = userEndpoints as unknown as Record<string, unknown>;

    expect(endpoints.noticeDetail).toBeUndefined();
  });

  it('unwraps the user notices page envelope to the first-page items array', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const controller = new AbortController();
    mock.onGet('/user/notices').reply(200, { items: [], total: 99 });

    await expect(
      userEndpoints.fetchNotices(client, { signal: controller.signal }),
    ).resolves.toEqual([]);
    expect(mock.history.get[0]?.signal).toBe(controller.signal);
  });

  it('parses dialect-v2 bare bodies as JSON requests without envelope unwrapping', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/login').reply(200, { is_admin: false, auth_data: 'jwt' });
    const result = await login(client, { email: 'a@b.c', password: 'x' });
    expect(result).toEqual({ is_admin: false, auth_data: 'jwt' });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      email: 'a@b.c',
      password: 'x',
    });
    expect(mock.history.post[0]?.headers?.['Content-Type']).toContain('application/json');
  });

  it('accepts the 201 register success on the dialect path', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/register').reply(201, { is_admin: false, auth_data: 'jwt' });

    await expect(
      passportEndpoints.register(client, { email: 'a@b.c', password: 'x' }),
    ).resolves.toMatchObject({ auth_data: 'jwt' });
  });

  it('parses dialect 204 empty successes as undefined', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/password-reset').reply(204);

    await expect(
      passportEndpoints.forget(client, { email: 'a@b.c', email_code: '123456', password: 'x' }),
    ).resolves.toBeUndefined();
  });

  it('sends the email-code request with the boolean is_forget field', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/email-codes').reply(204);

    await expect(
      passportEndpoints.sendEmailVerify(client, { email: 'a@b.c', is_forget: true }),
    ).resolves.toBeUndefined();

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      email: 'a@b.c',
      is_forget: true,
    });
  });

  it('rejects a successful HTTP response that violates a critical runtime contract', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/login').reply(200, { is_admin: false });

    await expect(login(client, { email: 'a@b.c', password: 'x' })).rejects.toBeInstanceOf(
      ApiContractError,
    );
  });

  it('rejects malformed core query payloads across guest, user, and admin surfaces', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const cases: Array<[string, () => Promise<unknown>, unknown]> = [
      ['guest config', () => fetchGuestConfig(client), { is_email_verify: 0 }],
      ['subscription', () => userEndpoints.getSubscribe(client), { plan_id: 1 }],
      ['plans', () => userEndpoints.fetchPlans(client), [{ id: 1, name: 'partial' }]],
      ['payment methods', () => userEndpoints.getPaymentMethod(client), [{ id: 1 }]],
      ['tickets', () => userEndpoints.fetchTickets(client), [{ id: 1 }]],
      ['servers', () => userEndpoints.fetchServers(client), [{ id: 1 }]],
      ['knowledge', () => userEndpoints.fetchKnowledge(client, 'zh-CN'), { Guide: [{ id: 1 }] }],
      ['admin payments', () => fetchAdminPayments(client), [{ id: 1 }]],
    ];

    for (const [_label, run, malformed] of cases) {
      mock.reset();
      mock.onAny().reply(200, { data: malformed });
      await expect(run()).rejects.toBeInstanceOf(ApiContractError);
    }
  });

  it('rejects an unknown admin server type before it can select a management endpoint', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.7 (W13): the modern nodes list is a bare dialect array.
    mock.onGet('/admin-path/nodes').reply(200, [
      {
        id: 1,
        name: 'unsupported node',
        group_id: [1],
        route_id: null,
        type: 'future-protocol',
        host: 'node.example.test',
        port: 443,
        server_port: null,
        show: true,
        rate: 1,
        parent_id: null,
        online: 0,
        last_check_at: null,
        last_push_at: null,
        available_status: 0,
        api_key: null,
        created_at: '2023-11-14T22:13:20Z',
        updated_at: '2023-11-14T22:13:20Z',
      },
    ]);

    await expect(fetchServerNodes(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('rejects checkout bodies that omit the §9.3 union kind tag', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/orders/T1/checkout').reply(200, { payload: 'https://pay.example.test' });

    await expect(
      userEndpoints.checkoutOrder(client, { trade_no: 'T1', method_id: 1 }),
    ).rejects.toBeInstanceOf(ApiContractError);
  });

  it('prepares Stripe PaymentIntent with the trade_no in the path and only method_id in the body', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const intent = {
      public_key: 'pk_test',
      client_secret: 'pi_test_secret_123',
      amount: 1234,
      currency: 'cny',
    };
    mock.onPost('/user/orders/T1/stripe-intent').reply(200, intent);

    await expect(
      userEndpoints.prepareStripePaymentIntent(client, { trade_no: 'T1', method_id: 5 }),
    ).resolves.toEqual(intent);
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({ method_id: 5 });
    expect(mock.history.post[0]?.data).not.toContain('token');
  });

  it('throws a typed checkout transport failure without owning UI presentation', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/orders/T1/checkout').networkError();

    await expect(
      userEndpoints.checkoutOrder(client, { trade_no: 'T1', method_id: 1 }),
    ).rejects.toMatchObject({
      status: 0,
      message: 'Network Error',
    });
  });

  it('throws typed transport failures for callers to present', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/test').networkError();

    await expect(
      client.request({ url: '/test', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 0, message: 'Network Error' });
  });

  it('unwraps the modern commissions page envelope into {data, total}', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/commissions?page=1&per_page=10').reply(200, { items: [], total: 0 });

    const result = await userEndpoints.inviteDetails(client, 1, 10);

    expect(result).toEqual({ data: [], total: 0 });
  });

  it('submits commission transfer amounts in cents from the API layer', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/commission-transfers').reply(204);

    await userEndpoints.transfer(client, '12.34');

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({ transfer_amount: 1234 });
  });

  it('converts decimal transfer amounts without binary floating-point drift', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/commission-transfers').reply(204);

    await userEndpoints.transfer(client, '19.99');

    // Binary multiplication produces 1998.9999…; the string-based boundary
    // conversion still sends the exact 1999 cents.
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({ transfer_amount: 1999 });
  });

  it('converts deposit major units to integer cents at the save-order boundary', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/orders').reply(201, { trade_no: 'DEPOSIT-1' });

    await expect(
      userEndpoints.saveOrder(client, {
        kind: 'deposit',
        deposit_amount: '12.34',
      }),
    ).resolves.toBe('DEPOSIT-1');

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      kind: 'deposit',
      deposit_amount: 1234,
    });
  });

  it('converts Admin user GiB and major-unit money exactly at the update boundary', async () => {
    // §6.6 (W12): the update is the JSON PATCH `users/{id}` — the id rides the
    // path, scaled fields cross as integer bytes/cents, never legacy
    // form-encoded `user/update`.
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPatch('/admin-path/users/7').reply(204);

    await updateUser(client, {
      id: 7,
      email: 'user@example.com',
      transfer_enable: '1.5',
      u: '0.0000000004656612873077392578125',
      d: 0,
      balance: '19.99',
      commission_balance: '-0.005',
    });

    expect(JSON.parse(String(mock.history.patch[0]?.data))).toEqual({
      email: 'user@example.com',
      transfer_enable: 1610612736,
      u: 1,
      d: 0,
      balance: 1999,
      commission_balance: -1,
    });
  });

  it('rejects unsafe Admin user scaling before issuing the update request', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
    });
    const mock = new AxiosMockAdapter(client.axios);

    await expect(
      updateUser(client, {
        id: 7,
        email: 'user@example.com',
        transfer_enable: '9007199254740992',
      }),
    ).rejects.toThrow(RangeError);
    expect(mock.history.patch).toHaveLength(0);
  });

  it('rejects an unsafe deposit amount before issuing the save-order request', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);

    await expect(
      userEndpoints.saveOrder(client, {
        kind: 'deposit',
        deposit_amount: '900719925474099.99',
      }),
    ).rejects.toThrow(RangeError);
    expect(mock.history.post).toHaveLength(0);
  });

  it('redeems a gift card as a dialect JSON request with the bare body', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/gift-card-redemptions').reply(200, { type: 1, value: 1234 });

    await expect(userEndpoints.redeemGiftCard(client, 'CARD-123')).resolves.toEqual({
      type: 1,
      value: 1234,
    });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({ giftcard: 'CARD-123' });
  });

  it('sends the Bearer scheme, Accept-Language, and JSON defaults on every request', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      getAuthData: () => 'auth',
      getLocale: () => 'zh-CN',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/commission-transfers').reply(204);
    await client.request({
      url: '/user/commission-transfers',
      method: 'POST',
      data: { transfer_amount: 100 },
      responseSchema: z.undefined(),
    });
    const request = mock.history.post[0]!;
    expect(JSON.parse(String(request.data))).toEqual({ transfer_amount: 100 });
    // §4.2: the stored value stays raw; the wire always carries the scheme.
    expect(request.headers?.authorization).toBe('Bearer auth');
    // §4.3: Accept-Language is the only locale signal — the W1→W14
    // transitional Content-Language copy died when W14 closed the wave series.
    expect(request.headers?.['Accept-Language']).toBe('zh-CN');
    expect(request.headers?.['Content-Language']).toBeUndefined();
    expect(request.withCredentials).toBe(false);
    expect(request.timeout).toBe(15_000);
  });

  it('allows deliberate cookie auth and long-running request timeout overrides', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      timeoutMs: 30_000,
      withCredentials: true,
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/slow').reply(200, {});

    await client.request({
      url: '/slow',
      method: 'GET',
      timeout: 60_000,
      responseSchema: z.unknown(),
    });

    expect(mock.history.get[0]?.withCredentials).toBe(true);
    expect(mock.history.get[0]?.timeout).toBe(60_000);
  });

  it('serializes array query params as repeated keys and omits nullish values', async () => {
    // §4.1: plain `key=value` scalars, repeated keys for arrays — never the
    // retired legacy recursive bracket encoding.
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onAny().reply(200, {});
    await client.request({
      url: '/test',
      method: 'GET',
      params: { reply_status: [0, 1], status: 0, email: undefined, cleared: null },
      responseSchema: z.unknown(),
    });
    expect(mock.history.get[0]?.url).toBe('/test?reply_status=0&reply_status=1&status=0');
  });

  it('sends explicit JSON nulls as §4.4 clears on the coupon PATCH', async () => {
    // §6.3 (W10): the full-form editor clears the nullable limit columns
    // with explicit JSON nulls (double-Option clear); an omitted window
    // field is simply absent (retain).
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPatch('/admin-path/coupons/5').reply(204);

    await updateCoupon(client, 5, {
      name: 'Edited',
      type: 2,
      value: '30',
      limit_use: null,
      started_at: 1_700_000_000,
      ended_at: undefined,
    });

    const body = JSON.parse(String(mock.history.patch[0]?.data)) as Record<string, unknown>;
    expect(body.limit_use).toBeNull();
    expect('ended_at' in body).toBe(false);
    expect(body.started_at).toBe('2023-11-14T22:13:20Z');
    expect(body.name).toBe('Edited');
    expect(body.value).toBe(30);
  });

  it('exchanges the one-time verify token via POST /auth/token-login', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/token-login').reply(200, { is_admin: false, auth_data: 'jwt' });

    await expect(tokenLogin(client, { verify: 'abc' })).resolves.toEqual({
      is_admin: false,
      auth_data: 'jwt',
    });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({ verify: 'abc' });
  });

  it('submits the step-up password as a dialect request and returns the typed grant', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/step-up').reply(200, { step_up_token: 'grant-token', expires_in: 900 });

    await expect(passportEndpoints.stepUp(client, { password: 'secret' })).resolves.toMatchObject({
      step_up_token: 'grant-token',
      expires_in: 900,
    });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({ password: 'secret' });
  });

  it('rides the step-up token on requests as the x-v2board-step-up header', async () => {
    const client = createApiClient({ baseURL: '/api/v1', getStepUpToken: () => 'grant-token' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(204);

    await client.request({ url: '/test', method: 'POST', responseSchema: z.undefined() });

    expect(mock.history.post[0]?.headers?.['x-v2board-step-up']).toBe('grant-token');
  });

  it('fires onUnauthorized only for the 401 session_expired problem', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(
      401,
      {
        type: 'about:blank',
        title: 'Unauthorized',
        status: 401,
        code: 'session_expired',
        detail: '未登录或登陆已过期',
      },
      { 'www-authenticate': 'Bearer' },
    );

    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 401, code: 'session_expired' });
    expect(onUnauthorized).toHaveBeenCalledOnce();
    expect(onUnauthorized.mock.calls[0]?.[0]).toBeInstanceOf(ApiProblemError);
  });

  it('keeps 403 permission and step-up problems session-preserving', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    const forbidden = (code: string, detail: string) => ({
      type: 'about:blank',
      title: 'Forbidden',
      status: 403,
      code,
      detail,
    });
    mock
      .onGet('/user/info')
      .replyOnce(403, forbidden('permission_denied', 'Permission denied'))
      .onGet('/user/info')
      .replyOnce(403, forbidden('step_up_required', 'Recent password verification is required'));

    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403, code: 'permission_denied' });
    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 403, code: 'step_up_required' });
    expect(onUnauthorized).not.toHaveBeenCalled();
  });

  it('no longer tears the session down on a legacy-shaped 403', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(403, { message: 'auth required' });
    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toBeInstanceOf(ApiError);
    expect(onUnauthorized).not.toHaveBeenCalled();
  });

  it('maps other non-2xx responses into ApiError without unauthorized handling', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(401, { message: 'auth required' });
    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toBeInstanceOf(ApiError);
    expect(onUnauthorized).not.toHaveBeenCalled();
  });

  it('surfaces dialect validation problems with detail, code, and the error bag', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/auth/login').reply(422, {
      type: 'about:blank',
      title: 'Unprocessable Entity',
      status: 422,
      code: 'validation_failed',
      detail: '邮箱格式不正确',
      errors: { email: ['邮箱格式不正确'] },
    });
    await expect(login(client, { email: '', password: '' })).rejects.toMatchObject({
      code: 'validation_failed',
      message: '邮箱格式不正确',
      errors: { email: ['邮箱格式不正确'] },
    });
  });

  it('falls back to the first non-dialect validation error as the ApiError message', async () => {
    // Non-problem `{message, errors}` bodies (gateway emulations, non-dialect
    // fixtures) still surface their most specific line as the message.
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/commission-transfers').reply(422, {
      message: 'validation failed',
      errors: { transfer_amount: ['Transfer amount cannot be empty'] },
    });
    await expect(
      client.request({
        url: '/user/commission-transfers',
        method: 'POST',
        data: { transfer_amount: '' },
        responseSchema: z.unknown(),
      }),
    ).rejects.toMatchObject({
      message: 'Transfer amount cannot be empty',
    });
  });

  it('prefixes admin paths when securePath is provided', () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'abc' });
    expect(client.resolveAdminPath('/tickets')).toBe('/abc/tickets');
  });

  it('mints the §8 page query and repeated reply_status keys for the admin ticket list', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.5 (W14): GET /tickets is the dialect-v2 `{items,total}` page; the
    // legacy JSON-stringified `reply_status` array param died with the flip,
    // and an empty `email` means "no filter" so it never reaches the wire.
    mock.onAny().reply(200, { items: [], total: 0 });

    await expect(
      fetchAdminTickets(client, { current: 2, pageSize: 10, reply_status: [0, 1], email: '' }),
    ).resolves.toEqual({ data: [], total: 0 });

    expect(mock.history.get[0]?.url).toBe(
      '/admin-path/tickets?page=2&per_page=10&reply_status=0&reply_status=1',
    );
  });

  it('reads the modern users page as {data,total} from the {items,total} body', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.6 (W12): GET /users is the dialect-v2 `{items,total}` page.
    mock.onGet('/admin-path/users').reply(200, { items: [], total: 0 });

    await expect(fetchUsers(client)).resolves.toEqual({ data: [], total: 0 });
    expect(
      mock.history.get.filter((request) => request.url?.startsWith('/admin-path/users')),
    ).toHaveLength(1);
  });

  it('reads the modern orders page as {data,total} from the {items,total} body', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.4 (W11): GET /orders is the dialect-v2 `{items,total}` page.
    mock.onGet('/admin-path/orders').reply(200, { items: [], total: 0 });

    await expect(fetchAdminOrders(client)).resolves.toEqual({ data: [], total: 0 });
    expect(mock.history.get.filter((request) => request.url?.includes('/orders'))).toHaveLength(1);
  });

  it('passes article id and cancellation through the admin knowledge-detail request', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const controller = new AbortController();
    // §6.3 (W10): GET /knowledge/{id} — bare dialect body, raw stored markdown.
    mock.onGet('/admin-path/knowledge/7').reply(200, {
      id: 7,
      category: 'Guide',
      title: 'Article',
      body: 'Body',
      language: 'zh-CN',
      sort: null,
      show: true,
      created_at: '2023-11-14T22:13:20Z',
      updated_at: '2023-11-14T22:13:20Z',
    });

    await expect(knowledgeDetail(client, 7, { signal: controller.signal })).resolves.toMatchObject({
      id: 7,
      title: 'Article',
    });
    expect(mock.history.get[0]?.signal).toBe(controller.signal);
  });

  it('normalizes fetched coupon and giftcard amount values to display units', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const couponRows = [
      makeCoupon({ id: 1, type: 1, value: 1234 }),
      makeCoupon({ id: 2, type: 2, value: 30 }),
    ];
    const giftcardRows = [
      makeGiftcard({ id: 1, type: 1, value: 5678 }),
      makeGiftcard({ id: 2, type: 3, value: 100 }),
    ];
    mock.onGet('/admin-path/coupons').reply(200, { items: couponRows, total: 2 });
    mock.onGet('/admin-path/gift-cards').reply(200, { items: giftcardRows, total: 2 });

    await expect(fetchAdminCoupons(client)).resolves.toMatchObject({
      items: [
        { id: 1, type: 1, value: 12.34 },
        { id: 2, type: 2, value: 30 },
      ],
      total: 2,
    });
    await expect(fetchAdminGiftcards(client)).resolves.toMatchObject({
      items: [
        { id: 1, type: 1, value: 56.78 },
        { id: 2, type: 3, value: 100 },
      ],
      total: 2,
    });
  });

  it('requests admin user traffic as the §8 stats/user-traffic page', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.8 (W14): RFC 3339 record_at, numeric server_rate, `{items,total}`.
    const record = { record_at: '2023-11-14T22:13:20Z', u: 1024, d: 2048, server_rate: 1 };
    mock
      .onGet('/admin-path/stats/user-traffic?user_id=1&page=2&per_page=10')
      .reply(200, { items: [record], total: 1 });

    await expect(statUser(client, { user_id: 1, current: 2, pageSize: 10 })).resolves.toEqual({
      data: [record],
      total: 1,
    });
    expect(mock.history.get[0]?.url).toBe(
      '/admin-path/stats/user-traffic?user_id=1&page=2&per_page=10',
    );
  });

  it('submits admin assigned-order amounts in cents as a dialect create', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.4 (W11): POST /orders (legacy `order/assign`) — 201 `{trade_no}`.
    mock.onPost('/admin-path/orders').reply(201, { trade_no: 'TRADE123' });

    await assignOrder(client, {
      email: 'user@example.com',
      plan_id: 1,
      period: 'month_price',
      total_amount: '12.34',
    });

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      email: 'user@example.com',
      plan_id: 1,
      period: 'month_price',
      total_amount: 1234,
    });
  });

  it('converts all admin money inputs at the API boundary without float drift', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.3 (W10): single creates are dialect JSON POSTs answered with 201 {id}.
    mock.onPost('/admin-path/coupons').reply(201, { id: 1 });
    mock.onPost('/admin-path/gift-cards').reply(201, { id: 1 });
    // §6.2 (W11): POST /payments is a dialect JSON create answered with 201 {id}.
    mock.onPost('/admin-path/payments').reply(201, { id: 1 });

    await generateCoupon(client, { type: 1, value: '19.99' });
    await generateGiftcard(client, { type: 1, value: '0.1' });
    await savePayment(client, {
      name: 'Card',
      payment: 'StripeCheckout',
      config: {},
      handling_fee_fixed: '1.05',
    });

    expect(JSON.parse(String(mock.history.post[0]?.data))).toMatchObject({ value: 1999 });
    expect(JSON.parse(String(mock.history.post[1]?.data))).toMatchObject({ value: 10 });
    expect(JSON.parse(String(mock.history.post[2]?.data))).toMatchObject({
      handling_fee_fixed: 105,
    });
  });

  it('sends only the dialect payment save contract with §4.4 create/edit empties', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.2 (W11): create POSTs the collection (201 {id}); edit PATCHes /{id} (204).
    mock.onPost('/admin-path/payments').reply(201, { id: 1 });
    mock.onPatch('/admin-path/payments/7').reply(204);

    await savePayment(client, {
      name: 'New gateway',
      payment: 'StripeCheckout',
      config: { secret_key: 'sk_new' },
      icon: '',
      notify_domain: '',
      handling_fee_fixed: '',
      handling_fee_percent: '',
      // Exercise the runtime whitelist as well as the stricter TS contract.
      uuid: 'must-not-round-trip',
      enable: 1,
    } as unknown as Parameters<typeof savePayment>[1]);

    // §4.4: on create an empty optional is absent (the documented default).
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      name: 'New gateway',
      payment: 'StripeCheckout',
      config: { secret_key: 'sk_new' },
    });

    await savePayment(client, {
      id: 7,
      name: 'Edited gateway',
      payment: 'StripeCheckout',
      config: { secret_key: 'sk_edited' },
      icon: '',
      notify_domain: '',
      handling_fee_fixed: '',
      handling_fee_percent: '',
    });

    // §4.4: on PATCH an empty optional is an explicit `null` clear.
    expect(JSON.parse(String(mock.history.patch[0]?.data))).toEqual({
      name: 'Edited gateway',
      payment: 'StripeCheckout',
      config: { secret_key: 'sk_edited' },
      icon: null,
      notify_domain: null,
      handling_fee_fixed: null,
      handling_fee_percent: null,
    });
  });

  it('merges the admin plan show/renew toggle into the dialect PATCH', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.2 (W11): the dedicated toggle action merged into PATCH /plans/{id} (204).
    mock.onPatch('/admin-path/plans/7').reply(204);

    await updatePlan(client, 7, 'renew', false);

    expect(JSON.parse(String(mock.history.patch[0]?.data))).toEqual({ renew: false });
  });

  it('uses the merged boolean-flag PATCH shape for admin plan toggles', () => {
    const source = readFileSync(new URL('./endpoints/admin.ts', import.meta.url), 'utf8');

    expect(source).toContain("key: 'show' | 'renew', value: boolean");
    expect(source).toContain('data: { [key]: value }');
    expect(source).not.toContain("adminPostTrue(client, '/plan/update', { id, [key]: value })");
  });

  it('normalizes modern admin user rows exactly like the packaged admin model', async () => {
    // §6.6 (W12): the list is GET /users `{items,total}` with RFC 3339 dates
    // and cents/byte integers; the detail is GET /users/{id} with the nested
    // `invite_user` object the normalizer flattens to `invite_user_email`.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const rawUser = {
      id: 1,
      email: 'user@example.com',
      password: '',
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
      expired_at: '2030-01-01T00:00:00Z',
      uuid: 'uuid',
      token: 'token',
      subscribe_url: 'https://example.com/sub',
      banned: 0,
      is_admin: 0,
      is_staff: 0,
      invite_user_id: null,
      discount: null,
      commission_rate: null,
      telegram_id: null,
      last_login_at: '2023-11-14T22:13:20Z',
      created_at: '2023-11-14T22:13:20Z',
      updated_at: '2023-11-14T22:13:20Z',
    };
    mock.onGet('/admin-path/users?page=1').reply(200, {
      items: [rawUser],
      total: 1,
    });
    mock
      .onGet('/admin-path/users/1')
      .reply(200, { ...rawUser, invite_user: { email: 'invite@example.com', id: 5 } });

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

    mock.resetHandlers();
    mock.onGet('/admin-path/users/1').reply(200, { ...rawUser, invite_user: null });
    const userWithoutInviter = await getUserInfoById(client, 1);
    expect(userWithoutInviter.invite_user).toBeNull();
    expect(userWithoutInviter).not.toHaveProperty('invite_user_email');
  });

  it('submits server node sort payloads as JSON to the modern route', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.7 (W13): route moves to POST /nodes/sort; the legacy grouped-map body is kept.
    const payload = { shadowsocks: { 1: 0, 3: 2 }, vmess: { 9: 1 } };
    mock.onPost('/admin-path/nodes/sort').reply(204);

    await sortServerNodes(client, payload);

    expect(mock.history.post[0]?.data).toBe(JSON.stringify(payload));
    expect(mock.history.post[0]?.headers?.['Content-Type']).toBe('application/json');
  });

  it('dispatches the admin server show toggle as a merged boolean PATCH', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.7 (W13): show/hide is a plain PATCH into the merged server update route.
    mock.onPatch('/admin-path/servers/vmess/8').reply(204);

    await showServer(client, 'vmess', 8, false);

    expect(JSON.parse(mock.history.patch[0]?.data ?? '{}')).toEqual({ show: false });
  });

  it('serializes admin server saves onto the typed §6.7 wire', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/servers/vmess').reply(201, { id: 12 });
    mock.onPatch('/admin-path/servers/anytls/5').reply(204);

    await saveServer(client, 'vmess', {
      name: 'Node',
      rate: '1.5',
      group_id: ['1', 2],
      route_id: null,
      host: 'node.example.test',
      port: '443',
      server_port: '8443',
      parent_id: '',
      show: '1',
      network: 'ws',
      tls: 1,
      // R22: the vmess protocol-settings keys stay camelCase on the modern wire.
      networkSettings: { path: '/ws' },
      tlsSettings: null,
    });
    expect(JSON.parse(mock.history.post[0]?.data ?? '{}')).toEqual({
      name: 'Node',
      rate: 1.5,
      group_id: [1, 2],
      route_id: null,
      host: 'node.example.test',
      port: 443,
      server_port: 8443,
      parent_id: null,
      show: true,
      network: 'ws',
      tls: 1,
      networkSettings: { path: '/ws' },
      tlsSettings: null,
    });

    await saveServer(client, 'anytls', {
      id: 5,
      name: 'AnyTLS',
      group_id: [1],
      host: 'anytls.example.test',
      port: 443,
      server_port: 443,
      rate: 1,
      padding_scheme: '["30-30"]',
    });
    expect(JSON.parse(mock.history.patch[0]?.data ?? '{}')).toEqual({
      name: 'AnyTLS',
      group_id: [1],
      host: 'anytls.example.test',
      port: 443,
      server_port: 443,
      rate: 1,
      padding_scheme: ['30-30'],
    });
  });

  it('submits admin plan prices in cents and strips fetched model metadata', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.2 (W11): the row id carries the PATCH path; the body is dialect JSON (204).
    mock.onPatch('/admin-path/plans/1').reply(204);

    await savePlan(client, {
      id: 1,
      name: '基础套餐',
      month_price: '12.34',
      quarter_price: 0,
      half_year_price: null,
      year_price: '',
      onetime_price: 300,
      force_update: true,
      show: 1,
      renew: 1,
      sort: 2,
      count: 12,
      created_at: 1_700_000_000,
      updated_at: 1_700_000_001,
    } as Parameters<typeof savePlan>[1] & Record<string, unknown>);

    // Prices serialize to cents; §4.4 turns an empty price into an explicit
    // null clear; the fetched-model metadata (show/renew/sort/count/timestamps)
    // is stripped by the save whitelist; `force_update` stays an edit-only flag.
    expect(JSON.parse(String(mock.history.patch[0]?.data))).toEqual({
      name: '基础套餐',
      month_price: 1234,
      quarter_price: 0,
      half_year_price: null,
      year_price: null,
      onetime_price: 30000,
      force_update: true,
    });
  });

  it('normalizes schema-validated admin plan prices from cents', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    // §6.2 (W11): GET /plans is the dialect-v2 bare array with cents prices.
    mock.onGet('/admin-path/plans').reply(200, [
      makePlan({
        month_price: 1234,
        quarter_price: null,
      }),
    ]);

    const result = await fetchPlans(client);

    expect(result[0]?.month_price).toBe(12.34);
    expect(result[0]?.quarter_price).toBeNull();
  });

  it('rejects malformed admin plan records instead of normalizing partial legacy data', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onGet('/admin-path/plans')
      .reply(200, [{ id: 1, name: 'Incomplete plan', month_price: 1234 }]);

    await expect(fetchPlans(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('rejects legacy comma-list config strings now that arrays are real arrays', async () => {
    // §6.1 (W9): the 0/1-flag, comma-list-string, and number-as-string
    // tolerances died with the dialect flip — a legacy-shaped body is a
    // contract violation, not something to silently renormalize.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    mock.onGet('/admin-path/config').reply(200, {
      ...config,
      invite: { ...config.invite, commission_withdraw_method: '支付宝,USDT' },
    });

    await expect(fetchConfig(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('normalizes only a null admin email template to the default template', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    mock.onGet('/admin-path/config').reply(200, {
      ...config,
      email: { ...config.email, email_template: null },
    });

    await expect(fetchConfig(client)).resolves.toMatchObject({
      email: { email_template: 'default' },
      email_template: 'default',
    });
  });

  it('preserves the exact commission_withdraw_limit decimal string', async () => {
    // §4.1 recorded exception: PostgreSQL NUMERIC round-trips lexically.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    mock.onGet('/admin-path/config').reply(200, {
      ...config,
      invite: {
        ...config.invite,
        commission_withdraw_limit: '9007199254740993.125',
      },
    });

    await expect(fetchConfig(client)).resolves.toMatchObject({
      commission_withdraw_limit: '9007199254740993.125',
    });
  });

  it('still rejects missing admin email-template and other malformed config fields', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    const emailWithoutTemplate: Record<string, unknown> = { ...config.email };
    delete emailWithoutTemplate.email_template;

    mock.onGet('/admin-path/config').replyOnce(200, { ...config, email: emailWithoutTemplate });
    mock.onGet('/admin-path/config').replyOnce(200, {
      ...config,
      site: { ...config.site, app_name: null },
    });

    await expect(fetchConfig(client)).rejects.toBeInstanceOf(ApiContractError);
    await expect(fetchConfig(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('PATCHes config as JSON with real arrays and reports full activation', async () => {
    // §6.1 (W9): the `'[]'`-string empty-array hack is dead — an empty array
    // rides as a real JSON `[]`; a bodiless 204 means fully activated.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPatch('/admin-path/config').reply(204);

    await expect(
      saveConfig(client, {
        email_whitelist_suffix: ['example.com', 'example.org'],
        commission_withdraw_method: [],
      }),
    ).resolves.toEqual({ activation: 'applied' });

    expect(JSON.parse(String(mock.history.patch[0]?.data))).toEqual({
      email_whitelist_suffix: ['example.com', 'example.org'],
      commission_withdraw_method: [],
    });
    expect(mock.history.patch[0]?.headers?.['Content-Type']).toContain('application/json');
  });

  it('surfaces the 202 activation-pending config save without treating it as an error', async () => {
    // §6.1: the write is durable — the caller must refetch, never resubmit.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPatch('/admin-path/config').reply(202, { activation: 'pending' });

    await expect(saveConfig(client, { app_name: 'Pending Site' })).resolves.toEqual({
      activation: 'pending',
    });
  });

  it('rejects a stale-revision config save with the 409 conflict problem code', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPatch('/admin-path/config').reply(
      409,
      {
        type: 'about:blank',
        title: 'Conflict',
        status: 409,
        code: 'config_revision_conflict',
        detail: '配置已被其他请求更新，请刷新后重试',
      },
      { 'content-type': 'application/problem+json' },
    );

    await expect(saveConfig(client, { app_name: 'Stale Site' })).rejects.toMatchObject({
      code: 'config_revision_conflict',
      status: 409,
    });
  });

  it('passes the modern group query when a page requests a single config group', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const site = makeAdminConfig().site;
    mock.onGet('/admin-path/config?group=site').reply(200, { site });

    await expect(fetchConfig(client, 'site')).resolves.toMatchObject({
      site: { currency: 'CNY' },
    });
  });

  it('keeps the admin notice fetch as a bare unpaginated array response', async () => {
    // §6.3 (W10): GET /notices deliberately stays unpaginated — the dialect
    // body is the bare array, no `{items, total}` page.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/notices').reply(200, [
      {
        id: 1,
        title: '维护通知',
        content: 'content',
        img_url: null,
        tags: ['system'],
        show: true,
        created_at: '2023-11-14T22:13:20Z',
        updated_at: '2023-11-14T22:13:20Z',
      },
    ]);

    await expect(fetchNotices(client)).resolves.toMatchObject([{ id: 1, title: '维护通知' }]);
    expect(mock.history.get[0]?.url).toBe('/admin-path/notices');
  });

  it('sets the telegram webhook with an empty JSON body when no token is given', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/telegram-webhook').reply(204);

    await expect(setTelegramWebhook(client)).resolves.toBeUndefined();

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({});
  });

  it('sets the telegram webhook with the explicit current token', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/telegram-webhook').reply(204);

    await setTelegramWebhook(client, 'current-token');

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      telegram_bot_token: 'current-token',
    });
  });

  it('sends the bodiless test-mail probe and parses the bare sent/log object', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/test-mail').reply(200, { sent: true, log: null });

    await expect(testSendMail(client)).resolves.toEqual({ sent: true, log: null });
    expect(mock.history.post[0]?.data).toBeUndefined();
  });

  it('encodes the system-log DSL query into filter/sort/pagination params', async () => {
    // §7 (W9): one JSON `filter` param, enum-validated sort scalars, §8
    // page/per_page — never legacy `filter[i][key]` brackets.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet(/^\/admin-path\/system\/logs\?/).reply(200, {
      items: [
        {
          id: 1,
          title: 'AlipayF2F notify failed',
          level: 'error',
          host: 'example.test',
          uri: '/api/v1/guest/payment/notify/alipay/uuid',
          method: 'POST',
          data: null,
          ip: '203.0.113.9',
          context: null,
          created_at: '2026-07-01T00:00:00Z',
          updated_at: '2026-07-01T00:00:00Z',
        },
      ],
      total: 1,
    });

    const page = await fetchSystemLogs(client, {
      page: 2,
      per_page: 10,
      filter: [{ field: 'level', op: 'eq', value: 'error' }],
      sort_by: 'created_at',
      sort_dir: 'desc',
    });

    expect(page.total).toBe(1);
    expect(page.items[0]).toMatchObject({ id: 1, level: 'error' });
    const url = String(mock.history.get[0]?.url);
    expect(url.startsWith('/admin-path/system/logs?')).toBe(true);
    const query = new URLSearchParams(url.slice(url.indexOf('?') + 1));
    expect(query.get('page')).toBe('2');
    expect(query.get('per_page')).toBe('10');
    expect(query.get('filter')).toBe('[{"field":"level","op":"eq","value":"error"}]');
    expect(query.get('sort_by')).toBe('created_at');
    expect(query.get('sort_dir')).toBe('desc');
    // Never the retired legacy bracket spelling.
    expect(url).not.toContain('filter%5B0%5D');
  });

  it('streams the byte-frozen generated user CSV from the dialect create route', async () => {
    // §6.6 (W12): bulk generate POSTs JSON to /users and streams the
    // byte-frozen credential CSV attachment — never legacy form-encoded
    // `user/generate`.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const csvBuffer = textBuffer('user-csv');
    mock.onPost('/admin-path/users').reply(200, csvBuffer, {
      'content-type': 'text/csv',
    });

    await expect(
      generateUser(client, {
        email_suffix: 'example.com',
        generate_count: '2',
      }),
    ).resolves.toMatchObject({ buffer: csvBuffer });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      email_suffix: 'example.com',
      generate_count: 2,
    });
    expect(mock.history.post[0]?.responseType).toBe('arraybuffer');
  });

  it('parses the single user create as the dialect 201 {id}', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/users').reply(201, { id: 42 });

    await expect(
      generateUser(client, { email_prefix: 'new', email_suffix: 'example.com' }),
    ).resolves.toMatchObject({ id: 42 });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      email_prefix: 'new',
      email_suffix: 'example.com',
    });
  });

  it('rejects a single user create whose JSON body omits the created id', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/users').reply(201, { data: 'unexpected' });

    await expect(generateUser(client, { email_suffix: 'example.com' })).rejects.toBeInstanceOf(
      ApiContractError,
    );
  });

  it('streams the byte-frozen dumped user CSV over the DSL export body', async () => {
    // §6.6 (W12): POST /users/export carries the §7 DSL `{filter}` clause
    // array, never the retired legacy `filter[i][key]` brackets.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const csvBuffer = textBuffer('users-csv');
    mock.onPost('/admin-path/users/export').reply(200, csvBuffer, {
      'content-type': 'text/csv',
    });

    await expect(
      dumpUsersCsv(client, [{ key: 'email', condition: '模糊', value: 'user@example.com' }]),
    ).resolves.toMatchObject({ buffer: csvBuffer });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      filter: [{ field: 'email', op: 'like', value: 'user@example.com' }],
    });
  });

  it('submits the required admin mail subject and content with its DSL target filter', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/users/mail').reply(204);

    await sendMailToUsers(client, {
      subject: 'Account notice',
      content: 'Please review your account.',
      filter: [{ key: 'email', condition: '模糊', value: 'user@example.com' }],
    });

    expect(JSON.parse(String(mock.history.post[0]?.data))).toEqual({
      subject: 'Account notice',
      content: 'Please review your account.',
      filter: [{ field: 'email', op: 'like', value: 'user@example.com' }],
    });
    expect(mock.history.post[0]?.headers?.['Idempotency-Key']).toEqual(expect.any(String));
  });

  it('reuses the admin mail idempotency key when one mutation request is retried', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/users/mail').replyOnce(500, {
      type: 'about:blank',
      title: 'Internal Server Error',
      status: 500,
      code: 'internal_error',
      detail: 'The mail batch could not be dispatched.',
    });
    mock.onPost('/admin-path/users/mail').reply(204);
    const mutation = { subject: 'Notice', content: 'Body' };

    await expect(sendMailToUsers(client, mutation)).rejects.toBeInstanceOf(ApiProblemError);
    await expect(sendMailToUsers(client, mutation)).resolves.toBeUndefined();

    const firstKey = mock.history.post[0]?.headers?.['Idempotency-Key'];
    const retryKey = mock.history.post[1]?.headers?.['Idempotency-Key'];
    expect(firstKey).toEqual(expect.any(String));
    expect(retryKey).toBe(firstKey);
    expect(mock.history.post[1]?.data).toBe(mock.history.post[0]?.data);

    await sendMailToUsers(client, { ...mutation });
    expect(mock.history.post[2]?.headers?.['Idempotency-Key']).not.toBe(firstKey);
  });

  it('preserves the byte-frozen coupon and gift-card bulk CSV buffers', async () => {
    // §6.3 (W10): `generate_count` keeps streaming the byte-frozen CSV
    // attachment from the dialect create routes.
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const couponBuffer = textBuffer('coupon-code');
    const giftcardBuffer = textBuffer('giftcard-code');
    mock.onPost('/admin-path/coupons').reply(200, couponBuffer, {
      'content-type': 'text/csv',
    });
    mock.onPost('/admin-path/gift-cards').reply(200, giftcardBuffer, {
      'content-type': 'text/csv',
    });

    await expect(generateCoupon(client, { generate_count: '2' })).resolves.toMatchObject({
      buffer: couponBuffer,
    });
    await expect(generateGiftcard(client, { generate_count: '2' })).resolves.toMatchObject({
      buffer: giftcardBuffer,
    });
    expect(JSON.parse(String(mock.history.post[0]?.data))).toMatchObject({ generate_count: 2 });
  });
});
