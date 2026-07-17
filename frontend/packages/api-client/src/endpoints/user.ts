import type {
  CommissionDetailPage,
  OrderCheckoutPayload,
  OrderCheckoutResult,
  OrderSavePayload,
  StripePaymentIntentPayload,
  TicketCreatePayload,
  TicketReplyPayload,
  TicketWithdrawPayload,
  UserUpdatePayload,
  UserStat,
} from '@v2board/types';
import type { ApiClient, ApiRequestConfig } from '../client';
import type { output } from 'zod';
import { decimalToCents } from '../money';
import {
  activeSessionMapSchema,
  arraySchema,
  availableServerSchema,
  booleanSchema,
  checkLoginSchema,
  checkoutEnvelopeSchema,
  commissionDetailSchema,
  couponSchema,
  inviteFetchSchema,
  knowledgeCategorySchema,
  knowledgeSchema,
  noticeSchema,
  numberSchema,
  orderSchema,
  ordersSchema,
  pageEnvelopeSchema,
  paymentMethodSchema,
  planSchema,
  redeemGiftCardEnvelopeSchema,
  stringSchema,
  stripePaymentIntentSchema,
  subscribeInfoSchema,
  telegramBotInfoSchema,
  ticketSchema,
  trafficLogSchema,
  trueSchema,
  userCommConfigSchema,
  userInfoSchema,
  userStatTupleSchema,
} from '../contracts';

type QueryRequestConfig = Pick<ApiRequestConfig, 'signal'>;

export const info = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/info',
    method: 'GET',
    responseSchema: userInfoSchema,
    ...config,
  });

export const getStat = (client: ApiClient, config?: QueryRequestConfig) =>
  // The backend returns a [pending_orders, pending_tickets, invited_count]
  // tuple; only the first two are surfaced in the UI.
  client
    .request({
      url: '/user/getStat',
      method: 'GET',
      responseSchema: userStatTupleSchema,
      ...config,
    })
    .then(([pending_orders, pending_tickets]): UserStat => ({
      pending_orders,
      pending_tickets,
    }));

export const checkLogin = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/checkLogin',
    method: 'GET',
    responseSchema: checkLoginSchema,
    ...config,
  });

export const getSubscribe = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/getSubscribe',
    method: 'GET',
    responseSchema: subscribeInfoSchema,
    ...config,
  });

export const update = (client: ApiClient, payload: UserUpdatePayload) =>
  client.request({
    url: '/user/update',
    method: 'POST',
    data: payload,
    responseSchema: trueSchema,
  });

export const changePassword = (client: ApiClient, old_password: string, new_password: string) =>
  client.request({
    url: '/user/changePassword',
    method: 'POST',
    data: { old_password, new_password },
    responseSchema: trueSchema,
  });

export const resetSecurity = (client: ApiClient) =>
  client.request({ url: '/user/resetSecurity', method: 'GET', responseSchema: stringSchema });

export const transfer = (client: ApiClient, transferAmount: number | string | undefined) =>
  client.request({
    url: '/user/transfer',
    method: 'POST',
    data: { transfer_amount: decimalToCents(transferAmount ?? '') },
    responseSchema: trueSchema,
  });

export const newPeriod = (client: ApiClient) =>
  client.request({ url: '/user/newPeriod', method: 'POST', responseSchema: trueSchema });

export type RedeemGiftCardResult = Pick<
  output<typeof redeemGiftCardEnvelopeSchema>,
  'type' | 'value'
>;

export const redeemGiftCard = async (
  client: ApiClient,
  giftcard: string,
): Promise<RedeemGiftCardResult> => {
  const env = await client.requestEnvelope({
    url: '/user/redeemgiftcard',
    method: 'POST',
    data: { giftcard },
    responseSchema: redeemGiftCardEnvelopeSchema,
  });
  return { type: env.type, value: env.value };
};

export const unbindTelegram = (client: ApiClient) =>
  client.request({ url: '/user/unbindTelegram', method: 'GET', responseSchema: trueSchema });

// Explicit sign-out: best-effort server-side revocation of the current opaque
// session (a Rust-only endpoint; the legacy API had no logout). The caller
// tears local auth down synchronously right after firing this, and the
// client's request interceptor reads the auth store on a microtask — after
// that teardown — so the bearer must be captured up front and passed as an
// explicit Authorization header. The backend treats a dead or absent bearer
// as a successful no-op.
export const logout = (client: ApiClient, config?: Pick<ApiRequestConfig, 'headers'>) =>
  client.request({
    url: '/user/logout',
    method: 'POST',
    responseSchema: trueSchema,
    ...config,
  });

export const getActiveSession = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/getActiveSession',
    method: 'GET',
    responseSchema: activeSessionMapSchema,
    ...config,
  });

export const removeActiveSession = (client: ApiClient, session_id: string) =>
  client.request({
    url: '/user/removeActiveSession',
    method: 'POST',
    data: { session_id },
    responseSchema: booleanSchema,
  });

export const fetchPlans = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/plan/fetch',
    method: 'GET',
    responseSchema: arraySchema(planSchema),
    ...config,
  });

export const fetchPlan = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/plan/fetch',
    method: 'GET',
    params: { id },
    responseSchema: planSchema,
    ...config,
  });

export const fetchOrders = (client: ApiClient, status?: number, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/order/fetch',
    method: 'GET',
    params: status === undefined ? {} : { status },
    responseSchema: ordersSchema,
    ...config,
  });

export const orderDetail = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/order/detail',
    method: 'GET',
    params: { trade_no },
    responseSchema: orderSchema,
    ...config,
  });

export type SaveOrderInput = Omit<OrderSavePayload, 'deposit_amount'> & {
  /** Deposit amount in major currency units; converted to integer cents at this boundary. */
  deposit_amount?: string;
};

export const saveOrder = async (client: ApiClient, payload: SaveOrderInput) => {
  const { deposit_amount, ...order } = payload;
  const data: OrderSavePayload =
    deposit_amount === undefined
      ? order
      : { ...order, deposit_amount: decimalToCents(deposit_amount) };
  return client.request({
    url: '/user/order/save',
    method: 'POST',
    data,
    responseSchema: stringSchema,
  });
};

export const checkoutOrder = async (
  client: ApiClient,
  payload: OrderCheckoutPayload,
): Promise<OrderCheckoutResult> => {
  const env = await client.requestEnvelope({
    url: '/user/order/checkout',
    method: 'POST',
    data: payload,
    responseSchema: checkoutEnvelopeSchema,
  });
  return { type: env.type, data: env.data };
};

export const checkOrder = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/order/check',
    method: 'GET',
    params: { trade_no },
    responseSchema: numberSchema,
    ...config,
  });

export const cancelOrder = (client: ApiClient, trade_no: string) =>
  client.request({
    url: '/user/order/cancel',
    method: 'POST',
    data: { trade_no },
    responseSchema: trueSchema,
  });

export const getPaymentMethod = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/order/getPaymentMethod',
    method: 'GET',
    responseSchema: arraySchema(paymentMethodSchema),
    ...config,
  });

export const generateInvite = (client: ApiClient) =>
  client.request({ url: '/user/invite/save', method: 'GET', responseSchema: booleanSchema });

export const fetchInvite = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/invite/fetch',
    method: 'GET',
    responseSchema: inviteFetchSchema,
    ...config,
  });

export const inviteDetails = async (
  client: ApiClient,
  current?: number,
  page_size?: number,
  config?: QueryRequestConfig,
): Promise<CommissionDetailPage> => {
  const env = await client.requestEnvelope({
    url: '/user/invite/details',
    method: 'GET',
    params: { current, page_size },
    responseSchema: pageEnvelopeSchema(commissionDetailSchema),
    ...config,
  });
  return { data: env.data, total: env.total };
};

export const fetchNotices = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/notice/fetch',
    method: 'GET',
    responseSchema: arraySchema(noticeSchema),
    ...config,
  });

export const fetchTickets = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/ticket/fetch',
    method: 'GET',
    responseSchema: arraySchema(ticketSchema),
    ...config,
  });

export const ticketDetail = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/ticket/fetch',
    method: 'GET',
    params: { id },
    responseSchema: ticketSchema,
    ...config,
  });

export const saveTicket = (client: ApiClient, payload: TicketCreatePayload) =>
  client.request({
    url: '/user/ticket/save',
    method: 'POST',
    data: payload,
    responseSchema: trueSchema,
  });

export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) =>
  client.request({
    url: '/user/ticket/reply',
    method: 'POST',
    data: payload,
    responseSchema: trueSchema,
  });

export const closeTicket = (client: ApiClient, id: number) =>
  client.request({
    url: '/user/ticket/close',
    method: 'POST',
    data: { id },
    responseSchema: trueSchema,
  });

export const withdrawTicket = (client: ApiClient, payload: TicketWithdrawPayload) =>
  client.request({
    url: '/user/ticket/withdraw',
    method: 'POST',
    data: payload,
    responseSchema: trueSchema,
  });

export const fetchServers = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/server/fetch',
    method: 'GET',
    responseSchema: arraySchema(availableServerSchema),
    ...config,
  });

export const checkCoupon = (client: ApiClient, code: string, plan_id: number | string) =>
  client.request({
    url: '/user/coupon/check',
    method: 'POST',
    data: { code, plan_id },
    responseSchema: couponSchema,
  });

export const getTelegramBotInfo = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/telegram/getBotInfo',
    method: 'GET',
    responseSchema: telegramBotInfoSchema,
    ...config,
  });

export const commConfig = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/comm/config',
    method: 'GET',
    responseSchema: userCommConfigSchema,
    ...config,
  });

export const prepareStripePaymentIntent = (
  client: ApiClient,
  payload: StripePaymentIntentPayload,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: '/user/order/stripe/intent',
    method: 'POST',
    data: payload,
    responseSchema: stripePaymentIntentSchema,
    ...config,
  });

export const fetchKnowledge = (
  client: ApiClient,
  language: string,
  keyword?: string,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: '/user/knowledge/fetch',
    method: 'GET',
    params: { language, keyword },
    responseSchema: knowledgeCategorySchema,
    signal: config?.signal,
  });

export const knowledgeDetail = (
  client: ApiClient,
  id: number | string,
  language: string,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: '/user/knowledge/fetch',
    method: 'GET',
    params: { id, language },
    responseSchema: knowledgeSchema,
    signal: config?.signal,
  });

export const getTrafficLog = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/stat/getTrafficLog',
    method: 'GET',
    responseSchema: arraySchema(trafficLogSchema),
    ...config,
  });
