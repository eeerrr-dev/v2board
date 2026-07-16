import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import AxiosMockAdapter from 'axios-mock-adapter';
import { z } from 'zod';
import { ApiContractError, ApiError, createApiClient, isStepUpRequiredError } from './client';
import { envelopeSchema, trueSchema } from './contracts';
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
  generateCoupon,
  generateGiftcard,
  generateUser,
  fetchPlans,
  knowledgeDetail,
  saveConfig,
  savePlan,
  savePayment,
  sendMailToUsers,
  setTelegramWebhook,
  sortServerNodes,
  statUser,
  testSendMail,
  updatePlan,
  updateUser,
  updateServer,
} from './endpoints/admin';
import { config as fetchGuestConfig } from './endpoints/guest';
import { login, token2Login } from './endpoints/passport';
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
    show: 1,
    limit_use: null,
    limit_use_with_user: null,
    limit_plan_ids: null,
    limit_period: null,
    started_at: 0,
    ended_at: 0,
    created_at: 0,
    updated_at: 0,
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
    used_user_ids: null,
    started_at: null,
    ended_at: null,
    created_at: 0,
    updated_at: 0,
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
    show: 1,
    sort: null,
    renew: 1,
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
    created_at: 0,
    updated_at: 0,
    ...overrides,
  };
}

function makeAdminConfig() {
  return {
    ticket: { ticket_status: 0 },
    deposit: { deposit_bounus: ['50:18', '100:38'] },
    invite: {
      invite_force: 1,
      invite_commission: 10,
      invite_gen_limit: 5,
      invite_never_expire: 0,
      commission_first_time_enable: 1,
      commission_auto_check_enable: 1,
      commission_withdraw_limit: 100,
      commission_withdraw_method: ['支付宝', 'USDT'],
      withdraw_close_enable: 0,
      commission_distribution_enable: 1,
      commission_distribution_l1: 50,
      commission_distribution_l2: 30,
      commission_distribution_l3: 20,
    },
    site: {
      logo: 'https://example.test/logo.png',
      force_https: 1,
      stop_register: 0,
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
    },
    subscribe: {
      plan_change_enable: 1,
      reset_traffic_method: 0,
      surplus_enable: 1,
      allow_new_period: 0,
      new_order_event_id: 1,
      renew_order_event_id: 0,
      change_order_event_id: 1,
      show_info_to_server_enable: 1,
      show_subscribe_method: 2,
      show_subscribe_expire: 30,
    },
    frontend: {
      frontend_theme_color: 'default',
      frontend_background_url: null,
      frontend_custom_html: null,
    },
    server: {
      server_api_url: 'https://node.example.test',
      server_token: 'token',
      server_pull_interval: 60,
      server_push_interval: 60,
      server_node_report_min_traffic: 0,
      server_device_online_min_traffic: 0,
      device_limit_mode: 0,
    },
    email: {
      email_template: 'default',
      email_host: 'smtp.example.test',
      email_port: '465',
      email_username: 'mailer',
      email_password: 'password',
      email_encryption: 'ssl',
      email_from_address: 'noreply@example.test',
    },
    telegram: {
      telegram_bot_enable: 1,
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
      email_verify: 1,
      safe_mode_enable: 1,
      secure_path: 'admin-path',
      email_whitelist_enable: 1,
      email_whitelist_suffix: ['qq.com', 'gmail.com'],
      email_gmail_limit_enable: 1,
      recaptcha_enable: 1,
      recaptcha_key: 'secret',
      recaptcha_site_key: 'site',
      register_limit_by_ip_enable: 1,
      register_limit_count: 3,
      register_limit_expire: 60,
      password_limit_enable: 1,
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

  it('fetches the active-session map with the backend contract shape', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const sessions = {
      'guid-1': {
        ip: '1.1.1.1',
        login_at: 1000,
        ua: 'Chrome',
        auth_data: '',
        current: true,
      },
      'guid-2': {
        ip: '2.2.2.2',
        login_at: 2000,
        ua: 'Firefox',
        auth_data: '',
        current: false,
      },
    };
    mock.onGet('/user/getActiveSession').reply(200, { data: sessions });

    await expect(userEndpoints.getActiveSession(client)).resolves.toEqual(sessions);
  });

  it('revokes an active session by posting its session_id', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/removeActiveSession').reply(200, { data: true });

    await expect(userEndpoints.removeActiveSession(client, 'guid-2')).resolves.toBe(true);

    expect(mock.history.post[0]?.data).toBe('session_id=guid-2');
  });

  it('does not expose a user notice detail endpoint absent from the original user bundle', () => {
    const endpoints = userEndpoints as unknown as Record<string, unknown>;

    expect(endpoints.noticeDetail).toBeUndefined();
  });

  it('keeps user notice fetch as the bundled notice model data-only state', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const controller = new AbortController();
    mock.onGet('/user/notice/fetch').reply(200, { data: [], total: 99 });

    await expect(
      userEndpoints.fetchNotices(client, { signal: controller.signal }),
    ).resolves.toEqual([]);
    expect(mock.history.get[0]?.signal).toBe(controller.signal);
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

  it('rejects a successful HTTP response that violates a critical runtime contract', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/passport/auth/login').reply(200, {
      data: { token: 't', is_admin: 0 },
    });

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
      ['admin config', () => fetchConfig(client, 'site'), { site: { currency: 'CNY' } }],
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
    mock.onGet('/admin-path/server/manage/getNodes').reply(200, {
      data: [
        {
          id: 1,
          name: 'unsupported node',
          group_id: [1],
          route_id: null,
          type: 'future-protocol',
          host: 'node.example.test',
          port: 443,
          server_port: null,
          show: 1,
          rate: '1',
          parent_id: null,
          online: 0,
          last_check_at: null,
        },
      ],
    });

    await expect(fetchServerNodes(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('treats legacy 200 HTTP responses with non-200 business code as failures', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/passport/auth/login').reply(200, { code: 500, message: 'invalid login' });
    await expect(login(client, { email: 'a@b.c', password: 'x' })).rejects.toMatchObject({
      status: 500,
      message: 'invalid login',
    });
  });

  it('adds the legacy HTTP 200 code to successful envelopes when the body omits it', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/notice/fetch').reply(200, { data: [], total: 0 });

    await expect(
      client.requestEnvelope({
        url: '/user/notice/fetch',
        method: 'GET',
        responseSchema: envelopeSchema(z.array(z.unknown())),
      }),
    ).resolves.toEqual({ code: 200, data: [], total: 0 });
  });

  it.each([
    ['code', '200'],
    ['total', '1'],
    ['type', null],
  ])('rejects an invalid legacy envelope %s field before returning data', async (field, value) => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/notice/fetch').reply(200, { data: [], [field]: value });

    await expect(
      client.requestEnvelope({
        url: '/user/notice/fetch',
        method: 'GET',
        responseSchema: envelopeSchema(z.array(z.string())),
      }),
    ).rejects.toBeInstanceOf(ApiContractError);
  });

  it('rejects checkout success envelopes that omit the required direct type field', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/order/checkout').reply(200, { data: 'https://pay.example.test' });

    await expect(
      userEndpoints.checkoutOrder(client, { trade_no: 'T1', method: 1 }),
    ).rejects.toBeInstanceOf(ApiContractError);
  });

  it('prepares Stripe PaymentIntent with only the order and selected method', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const intent = {
      public_key: 'pk_test',
      client_secret: 'pi_test_secret_123',
      amount: 1234,
      currency: 'cny',
    };
    mock.onPost('/user/order/stripe/intent').reply(200, { data: intent });

    await expect(
      userEndpoints.prepareStripePaymentIntent(client, { trade_no: 'T1', method: 5 }),
    ).resolves.toEqual(intent);
    expect(mock.history.post[0]?.data).toBe('trade_no=T1&method=5');
    expect(mock.history.post[0]?.data).not.toContain('token');
  });

  it('throws a typed checkout transport failure without owning UI presentation', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/order/checkout').networkError();

    await expect(
      userEndpoints.checkoutOrder(client, { trade_no: 'T1', method: 1 }),
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

  it('converts decimal transfer amounts without binary floating-point drift', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/transfer').reply(200, { data: true });

    await userEndpoints.transfer(client, '19.99');

    // Binary multiplication produces 1998.9999…; the string-based boundary
    // conversion still sends the exact 1999 cents.
    expect(mock.history.post[0]?.data).toBe('transfer_amount=1999');
  });

  it('converts deposit major units to integer cents at the save-order boundary', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/order/save').reply(200, { data: 'DEPOSIT-1' });

    await expect(
      userEndpoints.saveOrder(client, {
        plan_id: 0,
        period: 'deposit',
        deposit_amount: '12.34',
      }),
    ).resolves.toBe('DEPOSIT-1');

    expect(mock.history.post[0]?.data).toBe('plan_id=0&period=deposit&deposit_amount=1234');
  });

  it('converts Admin user GiB and major-unit money exactly at the update boundary', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
      nullFormValue: 'empty',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/user/update').reply(200, { data: true });

    await updateUser(client, {
      id: 7,
      email: 'user@example.com',
      transfer_enable: '1.5',
      u: '0.0000000004656612873077392578125',
      d: 0,
      balance: '19.99',
      commission_balance: '-0.005',
    });

    expect(Object.fromEntries(new URLSearchParams(String(mock.history.post[0]?.data)))).toEqual({
      id: '7',
      email: 'user@example.com',
      transfer_enable: '1610612736',
      u: '1',
      d: '0',
      balance: '1999',
      commission_balance: '-1',
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
    expect(mock.history.post).toHaveLength(0);
  });

  it('rejects an unsafe deposit amount before issuing the save-order request', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);

    await expect(
      userEndpoints.saveOrder(client, {
        plan_id: 0,
        period: 'deposit',
        deposit_amount: '900719925474099.99',
      }),
    ).rejects.toThrow(RangeError);
    expect(mock.history.post).toHaveLength(0);
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
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(201, { message: 'created' });

    await expect(
      client.request({ url: '/test', method: 'POST', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 201, message: 'created' });
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
    mock.onGet('/slow').reply(200, { data: true });

    await client.request({
      url: '/slow',
      method: 'GET',
      timeout: 60_000,
      responseSchema: trueSchema,
    });

    expect(mock.history.get[0]?.withCredentials).toBe(true);
    expect(mock.history.get[0]?.timeout).toBe(60_000);
  });

  it('omits nullish values from legacy form bodies', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(200, { data: true });
    await client.request({
      url: '/test',
      method: 'POST',
      data: { keep: 'yes', skipNull: null, skipUndefined: undefined },
      responseSchema: trueSchema,
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
      responseSchema: trueSchema,
    });

    expect(mock.history.post[0]?.data).toBe(
      'clear=&filter[0][key]=plan_id&filter[0][condition]=%3D&filter[0][value]=',
    );
  });

  it('distinguishes cleared from omitted coupon fields for the Rust retain-vs-clear contract', async () => {
    // The Rust coupon/giftcard editors gate each column on contains_key
    // (values.rs coupon_field_values): a present-but-empty form value clears
    // the column while an absent key retains the stored value. The 'empty'
    // null encoding plus undefined omission is what keeps that reachable.
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
      nullFormValue: 'empty',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/coupon/generate').reply(200, { data: true });

    await generateCoupon(client, {
      id: 5,
      name: 'Edited',
      limit_use: null,
      ended_at: undefined,
    });

    const params = new URLSearchParams(String(mock.history.post[0]?.data));
    expect(params.get('limit_use')).toBe('');
    expect(params.has('ended_at')).toBe(false);
    expect(params.get('name')).toBe('Edited');
  });

  it('uses legacy recursive bracket encoding for arrays and objects', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(200, { data: true });
    await client.request({
      url: '/test',
      method: 'POST',
      data: { list: ['a', 'b'], nested: { key: 'value' } },
      responseSchema: trueSchema,
    });
    expect(mock.history.post[0]?.data).toBe('list[0]=a&list[1]=b&nested[key]=value');
  });

  it('serializes only own enumerable form fields', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    const payload = Object.create({ inherited: 'legacy' }) as Record<string, unknown>;
    payload.own = 'value';

    mock.onPost('/test').reply(200, { data: true });
    await client.request({
      url: '/test',
      method: 'POST',
      data: payload,
      responseSchema: trueSchema,
    });

    expect(mock.history.post[0]?.data).toBe('own=value');
  });

  it('sends empty POST requests as legacy form requests', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/user/newPeriod').reply(200, { data: true });
    await client.request({
      url: '/user/newPeriod',
      method: 'POST',
      responseSchema: trueSchema,
    });
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

  it('submits the step-up password as a form request and returns the typed grant', async () => {
    const client = createApiClient({ baseURL: '/api/v1' });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onPost('/passport/auth/stepUp')
      .reply(200, { data: { step_up_token: 'grant-token', expires_in: 900 } });

    await expect(passportEndpoints.stepUp(client, { password: 'secret' })).resolves.toMatchObject({
      step_up_token: 'grant-token',
      expires_in: 900,
    });
    expect(mock.history.post[0]?.data).toBe('password=secret');
    expect(mock.history.post[0]?.headers?.['Content-Type']).toBe(
      'application/x-www-form-urlencoded',
    );
  });

  it('rides the step-up token on requests as the x-v2board-step-up header', async () => {
    const client = createApiClient({ baseURL: '/api/v1', getStepUpToken: () => 'grant-token' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/test').reply(200, { data: true });

    await client.request({ url: '/test', method: 'POST', responseSchema: trueSchema });

    expect(mock.history.post[0]?.headers?.['x-v2board-step-up']).toBe('grant-token');
  });

  it('recognizes only the exact step-up 403 as a step-up requirement', () => {
    const stepUp = new ApiError(403, 'Recent password verification is required');
    expect(isStepUpRequiredError(stepUp)).toBe(true);
    expect(isStepUpRequiredError(new ApiError(403, 'Permission denied'))).toBe(false);
    expect(
      isStepUpRequiredError(new ApiError(500, 'Recent password verification is required')),
    ).toBe(false);
    expect(isStepUpRequiredError(new Error('Recent password verification is required'))).toBe(
      false,
    );
  });

  it('maps 403 into ApiError and fires onUnauthorized', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(403, { message: 'auth required' });
    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toBeInstanceOf(ApiError);
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });

  it('maps non-200 envelope codes into ApiError without unauthorized handling', async () => {
    const onUnauthorized = vi.fn();
    const client = createApiClient({ baseURL: '/api/v1', onUnauthorized });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/user/info').reply(200, { code: 400, message: 'bad request' });

    await expect(
      client.request({ url: '/user/info', method: 'GET', responseSchema: z.unknown() }),
    ).rejects.toMatchObject({ status: 400, message: 'bad request' });
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
    expect(mock.history.get.filter((request) => request.url?.includes('/user/fetch'))).toHaveLength(
      1,
    );
    expect(
      mock.history.get.filter((request) => request.url?.includes('/order/fetch')),
    ).toHaveLength(1);
  });

  it('passes article id and cancellation through the admin knowledge-detail request', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const controller = new AbortController();
    mock.onGet('/admin-path/knowledge/fetch?id=7').reply(200, {
      data: {
        id: 7,
        category: 'Guide',
        title: 'Article',
        body: 'Body',
        language: 'zh-CN',
        sort: null,
        show: 1,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
      },
    });

    await expect(knowledgeDetail(client, 7, { signal: controller.signal })).resolves.toMatchObject({
      id: 7,
      title: 'Article',
    });
    expect(mock.history.get[0]?.signal).toBe(controller.signal);
  });

  it('normalizes fetched coupon and giftcard amount values to legacy model units', async () => {
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
    mock.onGet('/admin-path/coupon/fetch').reply(200, { data: couponRows, total: 2 });
    mock.onGet('/admin-path/giftcard/fetch').reply(200, { data: giftcardRows, total: 2 });

    await expect(fetchAdminCoupons(client)).resolves.toMatchObject({
      data: [
        { id: 1, type: 1, value: 12.34 },
        { id: 2, type: 2, value: 30 },
      ],
      total: 2,
    });
    await expect(fetchAdminGiftcards(client)).resolves.toMatchObject({
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
      expect(config.url).toBe('/admin-path/stat/getStatUser?user_id=1&current=2&pageSize=10');
      expect(config.params).toBeUndefined();
      return [
        200,
        {
          data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
          total: 1,
        },
      ];
    });

    await expect(statUser(client, { user_id: 1, current: 2, pageSize: 10 })).resolves.toEqual({
      data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
      total: 1,
    });
    expect(mock.history.get[0]?.url).toBe(
      '/admin-path/stat/getStatUser?user_id=1&current=2&pageSize=10',
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

  it('converts all admin money inputs at the API boundary without float drift', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost().reply(200, { data: true });

    await generateCoupon(client, { type: 1, value: '19.99' });
    await generateGiftcard(client, { type: 1, value: '0.1' });
    await savePayment(client, {
      name: 'Card',
      payment: 'StripeCheckout',
      config: {},
      handling_fee_fixed: '1.05',
    });

    expect(mock.history.post[0]?.data).toContain('value=1999');
    expect(mock.history.post[1]?.data).toContain('value=10');
    expect(mock.history.post[2]?.data).toContain('handling_fee_fixed=105');
  });

  it('sends only the PaymentController save contract and normalizes empty optionals', async () => {
    const client = createApiClient({
      baseURL: '/api/v1',
      adminSecurePath: () => 'admin-path',
      nullFormValue: 'empty',
    });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/payment/save').reply(200, { data: true });

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

    expect(Object.fromEntries(new URLSearchParams(String(mock.history.post[0]?.data)))).toEqual({
      name: 'New gateway',
      payment: 'StripeCheckout',
      'config[secret_key]': 'sk_new',
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

    expect(Object.fromEntries(new URLSearchParams(String(mock.history.post[1]?.data)))).toEqual({
      id: '7',
      name: 'Edited gateway',
      payment: 'StripeCheckout',
      'config[secret_key]': 'sk_edited',
      icon: '',
      notify_domain: '',
      handling_fee_fixed: '',
      handling_fee_percent: '',
    });
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
    expect(source).toContain('value: 0 | 1,');
    expect(source).toContain("adminPostTrue(client, '/plan/update', { id, [key]: value })");
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

    mock.resetHandlers();
    mock.onGet('/admin-path/user/getUserInfoById?id=1').reply(200, {
      data: { ...rawUser, invite_user: null },
    });
    const userWithoutInviter = await getUserInfoById(client, 1);
    expect(userWithoutInviter.invite_user).toBeNull();
    expect(userWithoutInviter).not.toHaveProperty('invite_user_email');
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
    expect(source).toContain('value: 0 | 1,');
    expect(source).toContain(
      'adminPostTrue(client, `/server/${type}/update`, { id, [key]: value })',
    );
    expect(source).not.toContain(
      'show: 0 | 1,\n) => adminPostTrue(client, `/server/${type}/update`, { id, show })',
    );
  });

  it('submits admin plan prices in cents and strips fetched model metadata', async () => {
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
      show: 1,
      renew: 1,
      sort: 2,
      count: 12,
      created_at: 1_700_000_000,
      updated_at: 1_700_000_001,
    } as Parameters<typeof savePlan>[1] & Record<string, unknown>);

    const body = new URLSearchParams(String(mock.history.post[0]?.data));
    expect(body.get('id')).toBe('1');
    expect(body.get('name')).toBe('基础套餐');
    expect(body.get('month_price')).toBe('1234');
    expect(body.get('quarter_price')).toBe('0');
    expect(body.get('year_price')).toBe('');
    expect(body.get('onetime_price')).toBe('30000');
    expect(body.get('force_update')).toBe('true');
    expect(body.get('half_year_price')).toBe('');
    expect(body.has('show')).toBe(false);
    expect(body.has('renew')).toBe(false);
    expect(body.has('sort')).toBe(false);
    expect(body.has('count')).toBe(false);
    expect(body.has('created_at')).toBe(false);
    expect(body.has('updated_at')).toBe(false);
  });

  it('normalizes schema-validated admin plan prices from cents', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/plan/fetch').reply(200, {
      data: [
        makePlan({
          month_price: 1234,
          quarter_price: null,
        }),
      ],
    });

    const result = await fetchPlans(client);

    expect(result[0]?.month_price).toBe(12.34);
    expect(result[0]?.quarter_price).toBeNull();
  });

  it('rejects malformed admin plan records instead of normalizing partial legacy data', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onGet('/admin-path/plan/fetch').reply(200, {
      data: [{ id: 1, name: 'Incomplete plan', month_price: 1234 }],
    });

    await expect(fetchPlans(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('normalizes legacy admin config comma-list strings after fetch', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    mock.onGet('/admin-path/config/fetch').reply(200, {
      data: {
        ...config,
        deposit: { deposit_bounus: '50:18,100:38' },
        invite: { ...config.invite, commission_withdraw_method: '支付宝,USDT' },
        site: { ...config.site, email_whitelist_suffix: 'qq.com,gmail.com' },
        safe: { ...config.safe, email_whitelist_suffix: 'safe.example' },
      },
    });

    const result = await fetchConfig(client);

    expect(result.deposit?.deposit_bounus).toEqual(['50:18', '100:38']);
    expect(result.deposit_bounus).toEqual(['50:18', '100:38']);
    expect(result.invite?.commission_withdraw_method).toEqual(['支付宝', 'USDT']);
    expect(result.commission_withdraw_method).toEqual(['支付宝', 'USDT']);
    expect(result.site?.email_whitelist_suffix).toEqual(['qq.com', 'gmail.com']);
    expect(result.email_whitelist_suffix).toEqual(['safe.example']);
  });

  it('normalizes only a null admin email template to the default template', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    mock.onGet('/admin-path/config/fetch').reply(200, {
      data: {
        ...config,
        email: { ...config.email, email_template: null },
      },
    });

    await expect(fetchConfig(client)).resolves.toMatchObject({
      email: { email_template: 'default' },
      email_template: 'default',
    });
  });

  it('preserves exact operator decimals as strings', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    mock.onGet('/admin-path/config/fetch').reply(200, {
      data: {
        ...config,
        invite: {
          ...config.invite,
          commission_withdraw_limit: '9007199254740993.125',
        },
        site: { ...config.site, try_out_hour: '0.1234567890123456789012345678' },
      },
    });

    await expect(fetchConfig(client)).resolves.toMatchObject({
      commission_withdraw_limit: '9007199254740993.125',
      try_out_hour: '0.1234567890123456789012345678',
    });
  });

  it('still rejects missing admin email-template and other malformed config fields', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const config = makeAdminConfig();
    const emailWithoutTemplate: Record<string, unknown> = { ...config.email };
    delete emailWithoutTemplate.email_template;

    mock.onGet('/admin-path/config/fetch').replyOnce(200, {
      data: { ...config, email: emailWithoutTemplate },
    });
    mock.onGet('/admin-path/config/fetch').replyOnce(200, {
      data: { ...config, site: { ...config.site, app_name: null } },
    });

    await expect(fetchConfig(client)).rejects.toBeInstanceOf(ApiContractError);
    await expect(fetchConfig(client)).rejects.toBeInstanceOf(ApiContractError);
  });

  it('preserves bracket encoding for populated config arrays and marks empty arrays explicitly', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/config/save').reply(200, { data: true });

    await saveConfig(client, {
      email_whitelist_suffix: ['example.com', 'example.org'],
      commission_withdraw_method: [],
    });

    expect(mock.history.post[0]?.data).toBe(
      'email_whitelist_suffix[0]=example.com&email_whitelist_suffix[1]=example.org&commission_withdraw_method=%5B%5D',
    );
  });

  it('passes the legacy config key when a page requests a single config group', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    const site = makeAdminConfig().site;
    mock.onAny().reply((config) => {
      expect(config.method).toBe('get');
      expect(config.url).toBe('/admin-path/config/fetch?key=site');
      expect(config.params).toBeUndefined();
      return [200, { data: { site } }];
    });

    await expect(fetchConfig(client, 'site')).resolves.toMatchObject({
      site: { currency: 'CNY' },
    });
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

  it('sets the telegram webhook with the explicit current token', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/config/setTelegramWebhook').reply(200, { data: true });

    await setTelegramWebhook(client, 'current-token');

    expect(mock.history.post[0]?.data).toBe('telegram_bot_token=current-token');
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
    mock
      .onPost('/admin-path/user/generate')
      .reply(200, textBuffer(JSON.stringify({ data: true })), {
        'content-type': 'application/json',
      });

    await expect(generateUser(client, { email_suffix: 'example.com' })).resolves.toMatchObject({
      code: 200,
      data: true,
    });
  });

  it('validates the JSON branch of the binary endpoint escape hatch', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock
      .onPost('/admin-path/user/generate')
      .reply(200, textBuffer(JSON.stringify({ data: 'unexpected' })), {
        'content-type': 'application/json',
      });

    await expect(generateUser(client, { email_suffix: 'example.com' })).rejects.toBeInstanceOf(
      ApiContractError,
    );
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

  it('submits the required admin mail subject and content with its target filter', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/user/sendMail').reply(200, { data: true });

    await sendMailToUsers(client, {
      subject: 'Account notice',
      content: 'Please review your account.',
      filter: [{ key: 'email', condition: '模糊', value: 'user@example.com' }],
    });

    expect(mock.history.post[0]?.data).toBe(
      'subject=Account%20notice&content=Please%20review%20your%20account.&filter[0][key]=email&filter[0][condition]=%E6%A8%A1%E7%B3%8A&filter[0][value]=user%40example.com',
    );
    expect(mock.history.post[0]?.headers?.['Idempotency-Key']).toEqual(expect.any(String));
  });

  it('reuses the admin mail idempotency key when one mutation request is retried', async () => {
    const client = createApiClient({ baseURL: '/api/v1', adminSecurePath: () => 'admin-path' });
    const mock = new AxiosMockAdapter(client.axios);
    mock.onPost('/admin-path/user/sendMail').replyOnce(500, { message: 'retry' });
    mock.onPost('/admin-path/user/sendMail').reply(200, { data: true });
    const mutation = { subject: 'Notice', content: 'Body' };

    await expect(sendMailToUsers(client, mutation)).rejects.toBeInstanceOf(ApiError);
    await expect(sendMailToUsers(client, mutation)).resolves.toBe(true);

    const firstKey = mock.history.post[0]?.headers?.['Idempotency-Key'];
    const retryKey = mock.history.post[1]?.headers?.['Idempotency-Key'];
    expect(firstKey).toEqual(expect.any(String));
    expect(retryKey).toBe(firstKey);
    expect(mock.history.post[1]?.data).toBe(mock.history.post[0]?.data);

    await sendMailToUsers(client, { ...mutation });
    expect(mock.history.post[2]?.headers?.['Idempotency-Key']).not.toBe(firstKey);
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
