import type {
  CommissionDetailPage,
  OrderCheckoutPayload,
  OrderCheckoutResult,
  OrderCreatePayload,
  PlanPeriod,
  StripePaymentIntentPayload,
  TicketCreatePayload,
  TicketReplyPayload,
  TicketWithdrawPayload,
  UserUpdatePayload,
  UserStat,
} from '@v2board/types';
import type { ApiClient, ApiRequestConfig } from '../client';
import type { output } from 'zod';
import { bearerAuthorization, pageSchema } from '../dialect';
import { decimalToCents } from '../money';
import {
  activeSessionMapSchema,
  arraySchema,
  availableServerSchema,
  booleanSchema,
  checkoutResultSchema,
  commissionDetailSchema,
  createdOrderSchema,
  inviteFetchSchema,
  knowledgeCategorySchema,
  knowledgeSchema,
  noContentSchema,
  noticeSchema,
  orderStatusSchema,
  pageEnvelopeSchema,
  paymentMethodSchema,
  redeemGiftCardEnvelopeSchema,
  sessionStateSchema,
  stringSchema,
  stripePaymentIntentSchema,
  subscribeInfoSchema,
  telegramBotInfoSchema,
  ticketSchema,
  trafficLogSchema,
  trueSchema,
  userCommConfigSchema,
  userCouponSchema,
  userInfoSchema,
  userOrderSchema,
  userOrdersSchema,
  userPlanSchema,
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

// Session probe (GET /auth/session, dialect v2 — the checkLogin successor).
// A dead or absent bearer is data ({is_login: false}), never a 401.
export const checkLogin = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/auth/session',
    method: 'GET',
    dialect: 'v2',
    responseSchema: sessionStateSchema,
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
// session (DELETE /auth/session, 204 — a Rust-only endpoint; the legacy API
// had no logout). The caller tears local auth down synchronously right after
// firing this, and the client's request interceptor reads the auth store on a
// microtask — after that teardown — so the raw auth_data must be captured up
// front and passed here; the endpoint puts the Bearer scheme on the wire
// (§4.2). The backend treats a dead or absent bearer as a successful no-op.
export const logout = (client: ApiClient, capturedAuthData?: string | null) => {
  const authorization = bearerAuthorization(capturedAuthData);
  return client.request({
    url: '/auth/session',
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
    ...(authorization ? { headers: { authorization } } : {}),
  });
};

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

/** GET /user/plans — dialect v2 bare array (docs/api-dialect.md §5.5, W4). */
export const fetchPlans = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/plans',
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(userPlanSchema),
    ...config,
  });

/** GET /user/plans/{id} — dialect v2 bare plan; a miss is 404 plan_not_found (§5.5, W4). */
export const fetchPlan = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  client.request({
    url: `/user/plans/${encodeURIComponent(id)}`,
    method: 'GET',
    dialect: 'v2',
    responseSchema: userPlanSchema,
    ...config,
  });

/** GET /user/orders?status= — dialect v2 bare array (§5.5, W4). */
export const fetchOrders = (client: ApiClient, status?: number, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/orders',
    method: 'GET',
    dialect: 'v2',
    params: status === undefined ? {} : { status },
    responseSchema: userOrdersSchema,
    ...config,
  });

/** GET /user/orders/{trade_no} — dialect v2 bare order (§5.5, W4). */
export const orderDetail = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  client.request({
    url: `/user/orders/${encodeURIComponent(trade_no)}`,
    method: 'GET',
    dialect: 'v2',
    responseSchema: userOrderSchema,
    ...config,
  });

export type SaveOrderInput =
  | { kind: 'plan'; plan_id: number; period: PlanPeriod; coupon_code?: string }
  | {
      kind: 'deposit';
      /** Deposit amount in major currency units; converted to integer cents at this boundary. */
      deposit_amount: string;
    };

/**
 * POST /user/orders — dialect v2 create from the §5.5 discriminated union,
 * answered 201 with `{trade_no}` (§9.4, W4); resolves to the bare trade_no
 * for navigation. The plan arm's `coupon_code` follows the §5.5 empty-coupon
 * rule: callers omit the field entirely when no coupon is applied.
 */
export const saveOrder = async (client: ApiClient, payload: SaveOrderInput) => {
  const data: OrderCreatePayload =
    payload.kind === 'deposit'
      ? { kind: 'deposit', deposit_amount: decimalToCents(payload.deposit_amount) }
      : payload;
  const created = await client.request({
    url: '/user/orders',
    method: 'POST',
    dialect: 'v2',
    data,
    responseSchema: createdOrderSchema,
  });
  return created.trade_no;
};

/** POST /user/orders/{trade_no}/checkout — dialect v2, the §9.3 result union (W4). */
export const checkoutOrder = (
  client: ApiClient,
  payload: OrderCheckoutPayload,
): Promise<OrderCheckoutResult> =>
  client.request({
    url: `/user/orders/${encodeURIComponent(payload.trade_no)}/checkout`,
    method: 'POST',
    dialect: 'v2',
    data: { method_id: payload.method_id },
    responseSchema: checkoutResultSchema,
  });

/**
 * GET /user/orders/{trade_no}/status — dialect v2 `{status}` body (§9.4, W4);
 * resolves to the bare status number the 3s poll consumes.
 */
export const checkOrder = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  client
    .request({
      url: `/user/orders/${encodeURIComponent(trade_no)}/status`,
      method: 'GET',
      dialect: 'v2',
      responseSchema: orderStatusSchema,
      ...config,
    })
    .then((body) => body.status);

/** POST /user/orders/{trade_no}/cancel — dialect v2, trade_no in the path, 204 (§5.5, W4). */
export const cancelOrder = (client: ApiClient, trade_no: string) =>
  client.request({
    url: `/user/orders/${encodeURIComponent(trade_no)}/cancel`,
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** GET /user/payment-methods — dialect v2 bare array, numeric percent (§5.5, W4). */
export const getPaymentMethod = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/payment-methods',
    method: 'GET',
    dialect: 'v2',
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

/**
 * GET /user/notices — dialect v2 (docs/api-dialect.md §5.8, W3): the
 * `{items, total}` page envelope with the server-side `per_page` default
 * pinned at 5. The client keeps requesting exactly the first default page,
 * so the `弹窗` auto-popup tag scan operates over the same notice universe
 * as legacy (Tier-1); consumers read the unwrapped items array.
 */
export const fetchNotices = async (client: ApiClient, config?: QueryRequestConfig) => {
  const page = await client.request({
    url: '/user/notices',
    method: 'GET',
    dialect: 'v2',
    responseSchema: pageSchema(noticeSchema),
    ...config,
  });
  return page.items;
};

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

/** POST /user/coupons/check — dialect v2 JSON `{code, plan_id}` → bare coupon (§5.5, W4). */
export const checkCoupon = (client: ApiClient, code: string, plan_id: number | string) =>
  client.request({
    url: '/user/coupons/check',
    method: 'POST',
    dialect: 'v2',
    data: { code, plan_id: Number(plan_id) },
    responseSchema: userCouponSchema,
  });

/** GET /user/telegram-bot — dialect v2 bare `{username}` (§5.3, W3). */
export const getTelegramBotInfo = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/telegram-bot',
    method: 'GET',
    dialect: 'v2',
    responseSchema: telegramBotInfoSchema,
    ...config,
  });

/** GET /user/config — dialect v2 bare body (§5.3, W3). */
export const commConfig = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/config',
    method: 'GET',
    dialect: 'v2',
    responseSchema: userCommConfigSchema,
    ...config,
  });

/**
 * POST /user/orders/{trade_no}/stripe-intent — dialect v2 bare intent body
 * (§5.5, W4). The Stripe PaymentIntent payloads behind it stay byte-frozen
 * (§2).
 */
export const prepareStripePaymentIntent = (
  client: ApiClient,
  payload: StripePaymentIntentPayload,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: `/user/orders/${encodeURIComponent(payload.trade_no)}/stripe-intent`,
    method: 'POST',
    dialect: 'v2',
    data: { method_id: payload.method_id },
    responseSchema: stripePaymentIntentSchema,
    ...config,
  });

/** GET /user/knowledge — dialect v2 bare `{category: [...]}` record (§5.8, W3). */
export const fetchKnowledge = (
  client: ApiClient,
  language: string,
  keyword?: string,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: '/user/knowledge',
    method: 'GET',
    dialect: 'v2',
    params: { language, keyword },
    responseSchema: knowledgeCategorySchema,
    signal: config?.signal,
  });

/**
 * GET /user/knowledge/{id} — dialect v2 bare article (§5.8, W3). The body is
 * non-idempotent (re-substituted per request), so same-id refetches stay
 * meaningful (Tier-1).
 */
export const knowledgeDetail = (
  client: ApiClient,
  id: number | string,
  language: string,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: `/user/knowledge/${id}`,
    method: 'GET',
    dialect: 'v2',
    params: { language },
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
