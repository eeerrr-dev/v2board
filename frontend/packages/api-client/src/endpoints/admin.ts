import type {
  AdminConfig,
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
  Plan,
  PlanPeriod,
  QueueStats,
  ServerRankItem,
  SystemStatus,
  Ticket,
  TicketReplyPayload,
  UserRankItem,
} from '@v2board/types';
import type { ApiClient } from '../client';

export interface AdminFilter {
  key: string;
  condition: string;
  value: string;
}

export interface AdminPageQuery {
  current?: number;
  pageSize?: number;
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
  filter?: AdminFilter[];
}

interface PageResult<T> {
  data: T[];
  total: number;
}

const adminGet = <T>(client: ApiClient, path: string, params?: Record<string, unknown>) =>
  client.request<T>({ url: client.resolveAdminPath(path), method: 'GET', params });

const adminGetEnvelope = <T>(
  client: ApiClient,
  path: string,
  params?: Record<string, unknown>,
) => client.requestEnvelope<T>({ url: client.resolveAdminPath(path), method: 'GET', params });

const adminPost = <T>(client: ApiClient, path: string, data?: Record<string, unknown>) =>
  client.request<T>({ url: client.resolveAdminPath(path), method: 'POST', data });

export const fetchConfig = (client: ApiClient) => adminGet<AdminConfig>(client, '/config/fetch');

export const saveConfig = (client: ApiClient, data: Partial<AdminConfig>) =>
  adminPost<true>(client, '/config/save', data as Record<string, unknown>);

export const getEmailTemplate = (client: ApiClient) =>
  adminGet<string[]>(client, '/config/getEmailTemplate');

export const getThemeTemplate = (client: ApiClient) =>
  adminGet<Record<string, unknown>>(client, '/config/getThemeTemplate');

export const setTelegramWebhook = (client: ApiClient, telegram_bot_token: string) =>
  adminPost<true>(client, '/config/setTelegramWebhook', { telegram_bot_token });

export const testSendMail = (client: ApiClient, email: string) =>
  adminPost<unknown>(client, '/config/testSendMail', { email });

export const fetchPlans = (client: ApiClient) => adminGet<Plan[]>(client, '/plan/fetch');

export const savePlan = (client: ApiClient, data: Partial<Plan> & { force_update?: 0 | 1 }) =>
  adminPost<true>(client, '/plan/save', data as Record<string, unknown>);

export const updatePlan = (client: ApiClient, id: number, show?: 0 | 1, renew?: 0 | 1) =>
  adminPost<true>(client, '/plan/update', { id, show, renew });

export const dropPlan = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/plan/drop', { id });

export const sortPlans = (client: ApiClient, plan_ids: number[]) =>
  adminPost<true>(client, '/plan/sort', { plan_ids });

export const fetchUsers = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<AdminUserRow>> => {
  const env = await adminGetEnvelope<AdminUserRow[]>(client, '/user/fetch', { ...query });
  return { data: env.data, total: env.total ?? 0 };
};

export const updateUser = (client: ApiClient, data: AdminUserUpdatePayload) =>
  adminPost<true>(client, '/user/update', data as unknown as Record<string, unknown>);

export const getUserInfoById = (client: ApiClient, id: number) =>
  adminGet<AdminUserRow>(client, '/user/getUserInfoById', { id });

export const generateUser = (
  client: ApiClient,
  data: {
    email_prefix?: string;
    email_suffix: string;
    password?: string;
    plan_id?: number;
    expired_at?: number;
    generate_count?: number;
  },
) => adminPost<true>(client, '/user/generate', data);

export const sendMailToUsers = (
  client: ApiClient,
  data: { subject: string; content: string; filter?: AdminFilter[] },
) => adminPost<true>(client, '/user/sendMail', data);

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
  const env = await adminGetEnvelope<AdminOrderRow[]>(client, '/order/fetch', { ...query });
  return { data: env.data, total: env.total ?? 0 };
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
  commission_status: 0 | 1 | 2 | 3,
) => adminPost<true>(client, '/order/update', { trade_no, commission_status });

export const assignOrder = (
  client: ApiClient,
  data: { email: string; plan_id: number; period: PlanPeriod; total_amount: number },
) => adminPost<string>(client, '/order/assign', data);

export const fetchPayments = (client: ApiClient) =>
  adminGet<AdminPayment[]>(client, '/payment/fetch');

export const paymentMethods = (client: ApiClient) =>
  adminGet<string[]>(client, '/payment/getPaymentMethods');

export const paymentForm = (client: ApiClient, payment: string) =>
  adminPost<PaymentFormDefinition>(client, '/payment/getPaymentForm', { payment });

export const savePayment = (client: ApiClient, data: Partial<AdminPayment>) =>
  adminPost<true>(client, '/payment/save', data as Record<string, unknown>);

export const showPayment = (client: ApiClient, id: number, enable: 0 | 1) =>
  adminPost<true>(client, '/payment/show', { id, enable });

export const sortPayments = (client: ApiClient, payment_ids: number[]) =>
  adminPost<true>(client, '/payment/sort', { payment_ids });

export const dropPayment = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/payment/drop', { id });

export const fetchNotices = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<Notice>> => {
  const env = await adminGetEnvelope<Notice[]>(client, '/notice/fetch', { ...query });
  return { data: env.data, total: env.total ?? 0 };
};

export const saveNotice = (client: ApiClient, data: Partial<Notice>) =>
  adminPost<true>(client, '/notice/save', data as Record<string, unknown>);

export const updateNotice = (client: ApiClient, data: Partial<Notice> & { id: number }) =>
  adminPost<true>(client, '/notice/update', data as Record<string, unknown>);

export const dropNotice = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/notice/drop', { id });

export const showNotice = (client: ApiClient, id: number, show: 0 | 1) =>
  adminPost<true>(client, '/notice/show', { id, show });

export const fetchTickets = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<Ticket>> => {
  const env = await adminGetEnvelope<Ticket[]>(client, '/ticket/fetch', { ...query });
  return { data: env.data, total: env.total ?? 0 };
};

export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) =>
  adminPost<true>(client, '/ticket/reply', payload as unknown as Record<string, unknown>);

export const closeTicket = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/ticket/close', { id });

export const fetchCoupons = async (
  client: ApiClient,
  query: AdminPageQuery = {},
): Promise<PageResult<Coupon>> => {
  const env = await adminGetEnvelope<Coupon[]>(client, '/coupon/fetch', { ...query });
  return { data: env.data, total: env.total ?? 0 };
};

export const generateCoupon = (
  client: ApiClient,
  data: Partial<Coupon> & { generate_count?: number },
) => adminPost<unknown>(client, '/coupon/generate', data as Record<string, unknown>);

export const dropCoupon = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/coupon/drop', { id });

export const showCoupon = (client: ApiClient, id: number, show: 0 | 1) =>
  adminPost<true>(client, '/coupon/show', { id, show });

export const fetchGiftcards = (client: ApiClient) =>
  adminGet<Giftcard[]>(client, '/giftcard/fetch');

export const generateGiftcard = (
  client: ApiClient,
  data: Partial<Giftcard> & { generate_count?: number },
) => adminPost<unknown>(client, '/giftcard/generate', data as Record<string, unknown>);

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

export const showKnowledge = (client: ApiClient, id: number, show: 0 | 1) =>
  adminPost<true>(client, '/knowledge/show', { id, show });

export const dropKnowledge = (client: ApiClient, id: number) =>
  adminPost<true>(client, '/knowledge/drop', { id });

export const sortKnowledge = (client: ApiClient, knowledge_ids: number[]) =>
  adminPost<true>(client, '/knowledge/sort', { knowledge_ids });

export const systemStatus = (client: ApiClient) =>
  adminGet<SystemStatus>(client, '/system/getSystemStatus');

export const queueStats = (client: ApiClient) =>
  adminGet<QueueStats>(client, '/system/getQueueStats');

export const systemLog = (client: ApiClient) => adminGet<string[]>(client, '/system/getSystemLog');

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

export const statUser = (client: ApiClient) =>
  adminGet<Record<string, number>>(client, '/stat/getStatUser');

export const themes = (client: ApiClient) =>
  adminGet<Record<string, unknown>>(client, '/theme/getThemes');

export const themeConfig = (client: ApiClient, name: string) =>
  adminPost<Record<string, unknown>>(client, '/theme/getThemeConfig', { name });

export const saveThemeConfig = (
  client: ApiClient,
  data: { name: string; config: Record<string, unknown> },
) => adminPost<true>(client, '/theme/saveThemeConfig', data as unknown as Record<string, unknown>);

export interface ServerNode {
  id: number;
  name: string;
  group_id: number[];
  route_id: number[];
  type: string;
  host: string;
  port: number;
  server_port: number | null;
  show: 0 | 1;
  rate: string;
  parent_id: number | null;
  online: number;
  last_check_at: number | null;
  is_online: 0 | 1;
}

export const fetchServerNodes = (client: ApiClient) =>
  adminGet<ServerNode[]>(client, '/server/manage/getNodes');

export const sortServerNodes = (client: ApiClient, ids: number[]) =>
  adminPost<true>(client, '/server/manage/sort', { ids });

export interface ServerGroup {
  id: number;
  name: string;
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
  match: string[];
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
  | 'anytls';

export const saveServer = (client: ApiClient, type: ServerTypeName, data: Record<string, unknown>) =>
  adminPost<true>(client, `/server/${type}/save`, data);

export const dropServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  adminPost<true>(client, `/server/${type}/drop`, { id });

export const updateServer = (
  client: ApiClient,
  type: ServerTypeName,
  id: number,
  show: 0 | 1,
) => adminPost<true>(client, `/server/${type}/update`, { id, show });

export const copyServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  adminPost<true>(client, `/server/${type}/copy`, { id });
