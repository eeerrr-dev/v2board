import type {
  AdminConfig,
  AdminConfigFlat,
  AdminOrderRow,
  AdminPayment,
  AdminStatSummary,
  AdminUserRow,
  AdminUserUpdatePayload,
  Coupon,
  Giftcard,
  Knowledge,
  KnowledgeSummary,
  Notice,
  OrderStatPoint,
  PaymentFormDefinition,
  QueueWorkloadItem,
  Plan,
  PlanPeriod,
  QueueStats,
  ServerRankItem,
  Ticket,
  TicketReplyPayload,
  UserRankItem,
} from '@v2board/types';
import type { ApiClient, ApiRequestConfig, BackendEnvelope } from '../client';

export interface AdminFilter {
  key: string;
  condition: string;
  value: string | number | null;
}

export interface AdminPageQuery {
  current?: number;
  pageSize?: number;
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
  filter?: AdminFilter[];
}

export interface AdminThemeField {
  field_name: string;
  field_type: 'select' | 'input' | 'textarea';
  label: string;
  placeholder?: string;
  select_options?: Record<string, string>;
}

export interface AdminThemeInfo {
  name: string;
  description: string;
  configs?: AdminThemeField[];
}

export interface AdminThemesResult {
  themes: Record<string, AdminThemeInfo>;
  active: string;
}

export interface AdminTestMailLog {
  error?: string;
  email?: string;
  config?: {
    host?: string;
    port?: string | number;
    encryption?: string;
    username?: string;
  };
}

export interface AdminTestMailResult extends BackendEnvelope<true> {
  log?: AdminTestMailLog;
}

export interface AdminUserTrafficRecord {
  record_at: number;
  u: number;
  d: number;
  server_rate: number | string;
}

export interface AdminUserTrafficQuery {
  user_id: number;
  page?: number;
  current?: number;
  pageSize?: number;
  total?: number;
}

interface PageResult<T> {
  data: T[];
  total?: number;
}

const LEGACY_GB_BYTES = 1_073_741_824;

const adminGet = <T>(client: ApiClient, path: string, params?: Record<string, unknown>) =>
  client.request<T>({ url: client.resolveAdminPath(path), method: 'GET', params });

const adminGetEnvelope = <T>(
  client: ApiClient,
  path: string,
  params?: Record<string, unknown>,
  config?: Pick<ApiRequestConfig, 'skipLegacyGlobalError'>,
) =>
  client.requestEnvelope<T>({
    url: client.resolveAdminPath(path),
    method: 'GET',
    params,
    ...config,
  });

const adminPost = <T>(client: ApiClient, path: string, data?: Record<string, unknown>) =>
  client.request<T>({ url: client.resolveAdminPath(path), method: 'POST', data });

function normalizeAdminConfig(config: AdminConfig): AdminConfig {
  const deposit = { ...(config.deposit ?? {}) } as Record<string, unknown>;
  const invite = { ...(config.invite ?? {}) } as Record<string, unknown>;
  const site = { ...(config.site ?? {}) } as Record<string, unknown>;

  if (typeof deposit.deposit_bounus === 'string') {
    deposit.deposit_bounus = deposit.deposit_bounus.split(',');
  }
  if (typeof invite.commission_withdraw_method === 'string') {
    invite.commission_withdraw_method = invite.commission_withdraw_method.split(',');
  }
  if (typeof site.email_whitelist_suffix === 'string') {
    site.email_whitelist_suffix = site.email_whitelist_suffix.split(',');
  }

  const normalizedConfig = {
    ...config,
    deposit: deposit as AdminConfig['deposit'],
    invite: invite as AdminConfig['invite'],
    site: site as AdminConfig['site'],
  };

  return {
    ...(normalizedConfig.ticket ?? {}),
    ...(normalizedConfig.deposit ?? {}),
    ...(normalizedConfig.invite ?? {}),
    ...(normalizedConfig.site ?? {}),
    ...(normalizedConfig.subscribe ?? {}),
    ...(normalizedConfig.frontend ?? {}),
    ...(normalizedConfig.server ?? {}),
    ...(normalizedConfig.email ?? {}),
    ...(normalizedConfig.telegram ?? {}),
    ...(normalizedConfig.app ?? {}),
    ...(normalizedConfig.safe ?? {}),
    ...normalizedConfig,
  };
}

export const fetchConfig = async (client: ApiClient, key?: string) =>
  normalizeAdminConfig(
    await adminGet<AdminConfig>(client, '/config/fetch', key ? { key } : undefined),
  );

export const saveConfig = (client: ApiClient, data: Partial<AdminConfigFlat>) =>
  adminPost<true>(client, '/config/save', data as Record<string, unknown>);

export const getEmailTemplate = (client: ApiClient) =>
  adminGet<string[]>(client, '/config/getEmailTemplate');

export const getThemeTemplate = (client: ApiClient) =>
  adminGet<Record<string, unknown>>(client, '/config/getThemeTemplate');

export const setTelegramWebhook = (client: ApiClient, telegram_bot_token?: string) =>
  adminPost<true>(client, '/config/setTelegramWebhook', { telegram_bot_token });

export const testSendMail = (client: ApiClient) =>
  client.requestEnvelope<true>({
    url: client.resolveAdminPath('/config/testSendMail'),
    method: 'POST',
  }) as Promise<AdminTestMailResult>;

const PLAN_PRICE_KEYS = [
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
] as const satisfies readonly (keyof Plan)[];

export type AdminPlanSavePayload = {
  [K in keyof Plan]?: Plan[K] | string | null;
} & {
  force_update?: boolean;
};

function normalizePlan(plan: Plan): Plan {
  const next = { ...plan };
  for (const key of PLAN_PRICE_KEYS) {
    const value = next[key];
    next[key] = value !== null ? ((Number(value) / 100) as Plan[typeof key]) : null;
  }
  return next;
}

function legacyScaledFixed(value: unknown, divisor: number) {
  return (Number(value) / divisor).toFixed(2);
}

function normalizeAdminUser(
  user: AdminUserRow,
  {
    includeInviteUserEmail = false,
    normalizeTotalUsed = false,
  }: { includeInviteUserEmail?: boolean; normalizeTotalUsed?: boolean } = {},
): AdminUserRow {
  const next = { ...user } as Record<string, unknown>;
  next.password = '';
  next.transfer_enable = legacyScaledFixed(next.transfer_enable, LEGACY_GB_BYTES);
  next.u = legacyScaledFixed(next.u, LEGACY_GB_BYTES);
  next.d = legacyScaledFixed(next.d, LEGACY_GB_BYTES);
  if (normalizeTotalUsed && 'total_used' in next) {
    next.total_used = legacyScaledFixed(next.total_used, LEGACY_GB_BYTES);
  }
  next.commission_balance = legacyScaledFixed(next.commission_balance, 100);
  next.balance = legacyScaledFixed(next.balance, 100);
  const inviteUser = next.invite_user as { email?: string } | undefined;
  if (includeInviteUserEmail && inviteUser) next.invite_user_email = inviteUser.email;
  return next as unknown as AdminUserRow;
}

export const fetchPlans = async (client: ApiClient) =>
  (await adminGet<Plan[]>(client, '/plan/fetch')).map(normalizePlan);

export const savePlan = (client: ApiClient, data: AdminPlanSavePayload) =>
  adminPost<true>(client, '/plan/save', serializePlanForSave(data));

function serializePlanForSave(data: AdminPlanSavePayload): Record<string, unknown> {
  const next: Record<string, unknown> = { ...data };
  for (const key of PLAN_PRICE_KEYS) {
    const value = next[key];
    if (value !== null) {
      next[key] = Math.round(100 * Number(value));
    }
  }
  return next;
}

export const updatePlan = (client: ApiClient, id: number, key: 'show' | 'renew', value: 0 | 1) =>
  adminPost<true>(client, '/plan/update', { id, [key]: value });

export const dropPlan = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/plan/drop', { id });

export const sortPlans = (client: ApiClient, plan_ids: number[]) =>
  adminPost<true>(client, '/plan/sort', { plan_ids });

export const fetchUsers = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<AdminUserRow>> => {
  const env = await adminGetEnvelope<AdminUserRow[]>(client, '/user/fetch', { ...query }, {
    skipLegacyGlobalError: true,
  });
  return {
    data: env.data.map((user) => normalizeAdminUser(user, { normalizeTotalUsed: true })),
    total: env.total,
  };
};

export const updateUser = (client: ApiClient, data: AdminUserUpdatePayload) =>
  adminPost<true>(client, '/user/update', data as unknown as Record<string, unknown>);

export const getUserInfoById = async (client: ApiClient, id: number) =>
  normalizeAdminUser(await adminGet<AdminUserRow>(client, '/user/getUserInfoById', { id }), {
    includeInviteUserEmail: true,
  });

export const generateUser = (
  client: ApiClient,
  data: {
    email_prefix?: string;
    email_suffix?: string;
    password?: string;
    plan_id?: number | null;
    expired_at?: number | string | null;
    generate_count?: number | string;
  },
) =>
  client.requestEnvelope<unknown>({
    url: client.resolveAdminPath('/user/generate'),
    method: 'POST',
    data,
    responseType: 'arraybuffer',
  }) as Promise<GenerateCsvResponse>;

export const sendMailToUsers = (
  client: ApiClient,
  data: { subject?: string; content?: string; filter?: AdminFilter[] },
) => adminPost<true>(client, '/user/sendMail', data);

export const dumpUsersCsv = (client: ApiClient, filter?: AdminFilter[]) =>
  client.requestEnvelope<unknown>({
    url: client.resolveAdminPath('/user/dumpCSV'),
    method: 'POST',
    data: { filter },
    responseType: 'arraybuffer',
  }) as Promise<GenerateCsvResponse>;

export const banUsers = (client: ApiClient, filter?: AdminFilter[]) =>
  adminPost<true>(client, '/user/ban', { filter });

export const resetUserSecret = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/user/resetSecret', { id });

export const deleteUser = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/user/delUser', { id });

export const deleteAllUsers = (client: ApiClient, filter?: AdminFilter[]) =>
  adminPost<true>(client, '/user/allDel', { filter });

export const fetchOrders = async (
  client: ApiClient,
  query: AdminPageQuery & { is_commission?: 0 | 1 } = {},
): Promise<PageResult<AdminOrderRow>> => {
  const env = await adminGetEnvelope<AdminOrderRow[]>(client, '/order/fetch', { ...query }, {
    skipLegacyGlobalError: true,
  });
  return { data: env.data, total: env.total };
};

export const orderDetail = (client: ApiClient, id: number) =>
  adminPost<AdminOrderRow>(client, '/order/detail', { id });

export const paidOrder = (client: ApiClient, trade_no: string) =>
  adminPost<true>(client, '/order/paid', { trade_no });

export const cancelOrder = (client: ApiClient, trade_no: string) =>
  adminPost<true>(client, '/order/cancel', { trade_no });

export const updateOrder = (
  client: ApiClient,
  trade_no: string,
  key: 'commission_status' | 'status',
  value: string | number,
) => adminPost<true>(client, '/order/update', { trade_no, [key]: value });

export const assignOrder = (
  client: ApiClient,
  data: {
    email?: string;
    plan_id?: number;
    period?: PlanPeriod;
    total_amount?: number | string | null;
  },
) =>
  adminPost<string>(client, '/order/assign', {
    ...data,
    total_amount: 100 * (data.total_amount as number),
  });

export const fetchPayments = (client: ApiClient) =>
  adminGet<AdminPayment[]>(client, '/payment/fetch');

export const paymentMethods = (client: ApiClient) =>
  adminGet<string[]>(client, '/payment/getPaymentMethods');

export type SavePaymentPayload = Omit<
  Partial<AdminPayment>,
  'config' | 'handling_fee_fixed' | 'handling_fee_percent'
> & {
  config?: Record<string, unknown>;
  handling_fee_fixed?: string | number | null;
  handling_fee_percent?: string | number | null;
};

export const paymentForm = (client: ApiClient, payment?: string, id?: number) =>
  adminPost<PaymentFormDefinition>(client, '/payment/getPaymentForm', { payment, id });

export const savePayment = (client: ApiClient, data: SavePaymentPayload) =>
  adminPost<true>(client, '/payment/save', data as Record<string, unknown>);

export const showPayment = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/payment/show', { id });

export const sortPayments = (client: ApiClient, payment_ids: number[]) =>
  adminPost<true>(client, '/payment/sort', { ids: payment_ids });

export const dropPayment = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/payment/drop', { id });

export const fetchNotices = async (
  client: ApiClient,
  _query: AdminPageQuery = {},
): Promise<PageResult<Notice>> => {
  const env = await adminGetEnvelope<Notice[]>(client, '/notice/fetch');
  return { data: env.data, total: env.total };
};

export const saveNotice = (client: ApiClient, data: Partial<Notice>) =>
  adminPost<true>(client, '/notice/save', data as Record<string, unknown>);

export const dropNotice = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/notice/drop', { id });

export const showNotice = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/notice/show', { id });

export const fetchTickets = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<Ticket>> => {
  const env = await adminGetEnvelope<Ticket[]>(client, '/ticket/fetch', { ...query });
  return { data: env.data, total: env.total };
};

export const ticketDetail = (client: ApiClient, id: number | string) =>
  adminGet<Ticket>(client, '/ticket/fetch', { id });

export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) =>
  adminPost<true>(client, '/ticket/reply', payload as unknown as Record<string, unknown>);

export const closeTicket = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/ticket/close', { id });

export const fetchCoupons = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<Coupon>> => {
  const env = await adminGetEnvelope<Coupon[]>(client, '/coupon/fetch', { ...query });
  env.data.forEach((coupon) => {
    if (coupon.type === 1) coupon.value = coupon.value / 100;
  });
  return { data: env.data, total: env.total };
};

export type GenerateCouponPayload = Omit<
  Partial<Coupon>,
  'value' | 'limit_use' | 'limit_use_with_user' | 'limit_plan_ids' | 'started_at' | 'ended_at'
> & {
  value?: number | string;
  limit_use?: number | string | null;
  limit_use_with_user?: number | string | null;
  limit_plan_ids?: Array<number | string> | null;
  started_at?: number | string | null;
  ended_at?: number | string | null;
  generate_count?: number | string;
};

export type GenerateCsvResponse = BackendEnvelope<unknown> & { buffer?: unknown };

export const generateCoupon = (client: ApiClient, data: GenerateCouponPayload) =>
  client.requestEnvelope<unknown>({
    url: client.resolveAdminPath('/coupon/generate'),
    method: 'POST',
    data: data as Record<string, unknown>,
    responseType: 'arraybuffer',
  }) as Promise<GenerateCsvResponse>;

export const dropCoupon = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/coupon/drop', { id });

export const showCoupon = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/coupon/show', { id });

export const fetchGiftcards = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<Giftcard>> => {
  const env = await adminGetEnvelope<Giftcard[]>(client, '/giftcard/fetch', { ...query });
  env.data.forEach((giftcard) => {
    if (giftcard.type === 1) giftcard.value = giftcard.value / 100;
  });
  return { data: env.data, total: env.total };
};

export type GenerateGiftcardPayload = Omit<
  Partial<Giftcard>,
  'value' | 'plan_id' | 'limit_use' | 'started_at' | 'ended_at'
> & {
  value?: number | string;
  plan_id?: number | string | null;
  limit_use?: number | string | null;
  started_at?: number | string | null;
  ended_at?: number | string | null;
  generate_count?: number | string;
};

export const generateGiftcard = (client: ApiClient, data: GenerateGiftcardPayload) =>
  client.requestEnvelope<unknown>({
    url: client.resolveAdminPath('/giftcard/generate'),
    method: 'POST',
    data: data as Record<string, unknown>,
    responseType: 'arraybuffer',
  }) as Promise<GenerateCsvResponse>;

export const dropGiftcard = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/giftcard/drop', { id });

export const fetchKnowledge = (client: ApiClient) =>
  adminGet<KnowledgeSummary[]>(client, '/knowledge/fetch');

export const knowledgeDetail = (client: ApiClient, id: number) =>
  adminGet<Knowledge>(client, '/knowledge/fetch', { id });

export const knowledgeCategories = (client: ApiClient) =>
  adminGet<string[]>(client, '/knowledge/getCategory');

export const saveKnowledge = (client: ApiClient, data: Partial<Knowledge>) =>
  adminPost<true>(client, '/knowledge/save', data as Record<string, unknown>);

export const showKnowledge = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/knowledge/show', { id });

export const dropKnowledge = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/knowledge/drop', { id });

export const sortKnowledge = (client: ApiClient, knowledge_ids: number[]) =>
  adminPost<true>(client, '/knowledge/sort', { knowledge_ids });

export const queueStats = (client: ApiClient) =>
  adminGet<QueueStats>(client, '/system/getQueueStats');

export const queueWorkload = (client: ApiClient) =>
  adminGet<QueueWorkloadItem[]>(client, '/system/getQueueWorkload');

export const statSummary = (client: ApiClient) =>
  adminGet<AdminStatSummary>(client, '/stat/getOverride');

export const statServerLastRank = (client: ApiClient) =>
  adminGet<ServerRankItem[]>(client, '/stat/getServerLastRank');

export const statServerTodayRank = (client: ApiClient) =>
  adminGet<ServerRankItem[]>(client, '/stat/getServerTodayRank');

export const statUserLastRank = (client: ApiClient) =>
  adminGet<UserRankItem[]>(client, '/stat/getUserLastRank');

export const statUserTodayRank = (client: ApiClient) =>
  adminGet<UserRankItem[]>(client, '/stat/getUserTodayRank');

export const statOrder = (client: ApiClient) =>
  adminGet<OrderStatPoint[]>(client, '/stat/getOrder');

export const statUser = async (
  client: ApiClient,
  query: AdminUserTrafficQuery,
): Promise<PageResult<AdminUserTrafficRecord>> => {
  const env = await adminGetEnvelope<AdminUserTrafficRecord[]>(client, '/stat/getStatUser', {
    ...query,
  });
  return { data: env.data, total: env.total };
};

export const themes = (client: ApiClient) =>
  adminGet<AdminThemesResult>(client, '/theme/getThemes');

export const themeConfig = (client: ApiClient, name: string) =>
  adminPost<Record<string, unknown>>(client, '/theme/getThemeConfig', { name });

export const saveThemeConfig = (client: ApiClient, data: { name: string; config: string }) =>
  adminPost<true>(client, '/theme/saveThemeConfig', data as unknown as Record<string, unknown>);

export interface ServerNode {
  id: number;
  name: string;
  group_id: number[] | string[];
  route_id: number[];
  type: string;
  host: string;
  port: number | string;
  server_port: number | null;
  show: 0 | 1 | string;
  rate: string;
  parent_id: number | null;
  online: number;
  last_check_at: number | null;
  is_online: 0 | 1;
  available_status?: 0 | 1 | 2;
}

export const fetchServerNodes = (client: ApiClient) =>
  adminGet<ServerNode[]>(client, '/server/manage/getNodes');

export const sortServerNodes = (
  client: ApiClient,
  payload: Record<string, Record<string | number, number>>,
) =>
  client.request<true>({
    url: client.resolveAdminPath('/server/manage/sort'),
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    data: JSON.stringify(payload),
  });

export interface ServerGroup {
  id: number;
  name: string;
  user_count?: number;
  server_count?: number;
  created_at: number;
  updated_at: number;
}

export const fetchServerGroups = (client: ApiClient) =>
  adminGet<ServerGroup[]>(client, '/server/group/fetch');

export const saveServerGroup = (client: ApiClient, data: Partial<ServerGroup>) =>
  adminPost<true>(client, '/server/group/save', data as Record<string, unknown>);

export const dropServerGroup = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/server/group/drop', { id });

export interface ServerRoute {
  id: number;
  remarks: string;
  match: string[] | string;
  action: string;
  action_value: string | null;
  created_at: number;
  updated_at: number;
}

export const fetchServerRoutes = (client: ApiClient) =>
  adminGet<ServerRoute[]>(client, '/server/route/fetch');

export const saveServerRoute = (client: ApiClient, data: Partial<ServerRoute>) =>
  adminPost<true>(client, '/server/route/save', data as Record<string, unknown>);

export const dropServerRoute = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/server/route/drop', { id });

export type ServerTypeName =
  | 'shadowsocks'
  | 'vmess'
  | 'trojan'
  | 'tuic'
  | 'vless'
  | 'hysteria'
  | 'anytls'
  | 'v2node';

export const saveServer = (
  client: ApiClient,
  type: ServerTypeName,
  data: Record<string, unknown>,
) => adminPost<true>(client, `/server/${type}/save`, data);

export const dropServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  adminPost<true>(client, `/server/${type}/drop`, { id });

export const updateServer = (
  client: ApiClient,
  type: ServerTypeName,
  id: number,
  key: 'show',
  value: 0 | 1,
) => adminPost<true>(client, `/server/${type}/update`, { id, [key]: value });

export const copyServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  adminPost<true>(client, `/server/${type}/copy`, { id });
