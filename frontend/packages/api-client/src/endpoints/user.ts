import type {
  ActiveSession,
  AvailableServer,
  CheckLoginResult,
  Coupon,
  CommissionDetailPage,
  InviteFetchResult,
  Knowledge,
  KnowledgeCategory,
  Notice,
  NoticePage,
  Order,
  OrderCheckoutPayload,
  OrderCheckoutResult,
  OrderSavePayload,
  PaymentMethod,
  Plan,
  SubscribeInfo,
  Ticket,
  TicketCreatePayload,
  TicketReplyPayload,
  TicketWithdrawPayload,
  TrafficLogEntry,
  UserCommConfig,
  UserInfo,
  UserUpdatePayload,
  UserStat,
} from '@v2board/types';
import type { ApiClient } from '../client';

export const info = (client: ApiClient) =>
  client.request<UserInfo>({ url: '/user/info', method: 'GET' });

export const getStat = (client: ApiClient) =>
  client.request<[number, number, number]>({ url: '/user/getStat', method: 'GET' }).then(
    ([pending_orders, pending_tickets, invited_count]): UserStat => ({
      pending_orders,
      pending_tickets,
      invited_count,
    }),
  );

export const checkLogin = (client: ApiClient) =>
  client.request<CheckLoginResult>({ url: '/user/checkLogin', method: 'GET' });

export const getSubscribe = (client: ApiClient) =>
  client.request<SubscribeInfo>({ url: '/user/getSubscribe', method: 'GET' });

export const update = (client: ApiClient, payload: UserUpdatePayload) =>
  client.request<true>({ url: '/user/update', method: 'POST', data: payload });

export const changePassword = (client: ApiClient, old_password: string, new_password: string) =>
  client.request<true>({
    url: '/user/changePassword',
    method: 'POST',
    data: { old_password, new_password },
  });

export const resetSecurity = (client: ApiClient) =>
  client.request<string>({ url: '/user/resetSecurity', method: 'GET' });

export const transfer = (client: ApiClient, transfer_amount: number) =>
  client.request<true>({ url: '/user/transfer', method: 'POST', data: { transfer_amount } });

export const newPeriod = (client: ApiClient) =>
  client.request<true>({ url: '/user/newPeriod', method: 'POST' });

export interface RedeemGiftCardResult {
  type: number;
  value: number;
}

export const redeemGiftCard = async (
  client: ApiClient,
  giftcard: string,
): Promise<RedeemGiftCardResult> => {
  const env = await client.requestEnvelope<true>({
    url: '/user/redeemgiftcard',
    method: 'POST',
    data: { giftcard },
  });
  const raw = env as unknown as { type: number; value: number };
  return { type: raw.type, value: raw.value };
};

export const unbindTelegram = (client: ApiClient) =>
  client.request<true>({ url: '/user/unbindTelegram', method: 'GET' });

export const getActiveSession = (client: ApiClient) =>
  client.request<ActiveSession[]>({ url: '/user/getActiveSession', method: 'GET' });

export const removeActiveSession = (client: ApiClient, session_id: string) =>
  client.request<boolean>({
    url: '/user/removeActiveSession',
    method: 'POST',
    data: { session_id },
  });

export const fetchPlans = (client: ApiClient) =>
  client.request<Plan[]>({ url: '/user/plan/fetch', method: 'GET' });

export const fetchPlan = (client: ApiClient, id: number) =>
  client.request<Plan>({ url: '/user/plan/fetch', method: 'GET', params: { id } });

export const fetchOrders = (client: ApiClient, status?: number) =>
  client.request<Order[]>({
    url: '/user/order/fetch',
    method: 'GET',
    params: status === undefined ? {} : { status },
  });

export const orderDetail = (client: ApiClient, trade_no: string) =>
  client.request<Order>({ url: '/user/order/detail', method: 'GET', params: { trade_no } });

export const saveOrder = (client: ApiClient, payload: OrderSavePayload) =>
  client.request<string>({ url: '/user/order/save', method: 'POST', data: payload });

export const checkoutOrder = async (
  client: ApiClient,
  payload: OrderCheckoutPayload,
): Promise<OrderCheckoutResult> => {
  const env = await client.requestEnvelope<OrderCheckoutResult['data']>({
    url: '/user/order/checkout',
    method: 'POST',
    data: payload,
  });
  return { type: (env.type ?? 0) as OrderCheckoutResult['type'], data: env.data };
};

export const checkOrder = (client: ApiClient, trade_no: string) =>
  client.request<number>({ url: '/user/order/check', method: 'GET', params: { trade_no } });

export const cancelOrder = (client: ApiClient, trade_no: string) =>
  client.request<true>({ url: '/user/order/cancel', method: 'POST', data: { trade_no } });

export const getPaymentMethod = (client: ApiClient) =>
  client.request<PaymentMethod[]>({ url: '/user/order/getPaymentMethod', method: 'GET' });

export const generateInvite = (client: ApiClient) =>
  client.request<boolean>({ url: '/user/invite/save', method: 'GET' });

export const fetchInvite = (client: ApiClient) =>
  client.request<InviteFetchResult>({ url: '/user/invite/fetch', method: 'GET' });

export const inviteDetails = async (
  client: ApiClient,
  current = 1,
  page_size = 10,
): Promise<CommissionDetailPage> => {
  const env = await client.requestEnvelope<CommissionDetailPage['data']>({
    url: '/user/invite/details',
    method: 'GET',
    params: { current, page_size },
  });
  return { data: env.data, total: env.total ?? 0 };
};

export const fetchNotices = async (client: ApiClient): Promise<NoticePage> => {
  const env = await client.requestEnvelope<Notice[]>({
    url: '/user/notice/fetch',
    method: 'GET',
  });
  return { data: env.data, total: env.total ?? 0 };
};

export const noticeDetail = (client: ApiClient, id: number) =>
  client.request<Notice>({ url: '/user/notice/fetch', method: 'GET', params: { id } });

export const fetchTickets = (client: ApiClient) =>
  client.request<Ticket[]>({ url: '/user/ticket/fetch', method: 'GET' });

export const ticketDetail = (client: ApiClient, id: number) =>
  client.request<Ticket>({ url: '/user/ticket/fetch', method: 'GET', params: { id } });

export const saveTicket = (client: ApiClient, payload: TicketCreatePayload) =>
  client.request<true>({ url: '/user/ticket/save', method: 'POST', data: payload });

export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) =>
  client.request<true>({ url: '/user/ticket/reply', method: 'POST', data: payload });

export const closeTicket = (client: ApiClient, id: number) =>
  client.request<true>({ url: '/user/ticket/close', method: 'POST', data: { id } });

export const withdrawTicket = (client: ApiClient, payload: TicketWithdrawPayload) =>
  client.request<true>({ url: '/user/ticket/withdraw', method: 'POST', data: payload });

export const fetchServers = (client: ApiClient) =>
  client.request<AvailableServer[]>({ url: '/user/server/fetch', method: 'GET' });

export const checkCoupon = (client: ApiClient, code: string, plan_id: number) =>
  client.request<Coupon>({
    url: '/user/coupon/check',
    method: 'POST',
    data: { code, plan_id },
  });

export const getTelegramBotInfo = (client: ApiClient) =>
  client.request<{ username: string }>({ url: '/user/telegram/getBotInfo', method: 'GET' });

export const commConfig = (client: ApiClient) =>
  client.request<UserCommConfig>({ url: '/user/comm/config', method: 'GET' });

export const getStripePublicKey = (client: ApiClient, id: number) =>
  client.request<string>({
    url: '/user/comm/getStripePublicKey',
    method: 'POST',
    data: { id },
  });

export const fetchKnowledge = (client: ApiClient, language: string, keyword?: string) =>
  client.request<KnowledgeCategory>({
    url: '/user/knowledge/fetch',
    method: 'GET',
    params: { language, keyword },
  });

export const knowledgeDetail = (client: ApiClient, id: number, language: string) =>
  client.request<Knowledge>({ url: '/user/knowledge/fetch', method: 'GET', params: { id, language } });

export const getTrafficLog = (client: ApiClient) =>
  client.request<TrafficLogEntry[]>({ url: '/user/stat/getTrafficLog', method: 'GET' });
