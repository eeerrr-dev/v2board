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
  activeSessionSchema,
  arraySchema,
  availableServerSchema,
  checkoutResultSchema,
  commissionDetailSchema,
  createdOrderSchema,
  giftCardRedemptionSchema,
  inviteFetchSchema,
  knowledgeCategorySchema,
  knowledgeSchema,
  noContentSchema,
  noticeSchema,
  orderStatusSchema,
  paymentMethodSchema,
  resetSubscribeTokenSchema,
  sessionStateSchema,
  stripePaymentIntentSchema,
  subscriptionSchema,
  telegramBotInfoSchema,
  ticketSchema,
  trafficLogSchema,
  trueSchema,
  userCommConfigSchema,
  userCouponSchema,
  userOrderSchema,
  userOrdersSchema,
  userPlanSchema,
  userProfileSchema,
  userStatsSchema,
} from '../contracts';

type QueryRequestConfig = Pick<ApiRequestConfig, 'signal'>;

/** GET /user/profile — dialect v2 bare profile (docs/api-dialect.md §5.3, W5). */
export const info = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/profile',
    method: 'GET',
    dialect: 'v2',
    responseSchema: userProfileSchema,
    ...config,
  });

/** GET /user/stats — dialect v2 named counts (§9.1, W5). */
export const getStat = (client: ApiClient, config?: QueryRequestConfig): Promise<UserStat> =>
  client.request({
    url: '/user/stats',
    method: 'GET',
    dialect: 'v2',
    responseSchema: userStatsSchema,
    ...config,
  });

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

/** GET /user/subscription — dialect v2 bare body, explicit-null plan (§5.4, W5). */
export const getSubscribe = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/subscription',
    method: 'GET',
    dialect: 'v2',
    responseSchema: subscriptionSchema,
    ...config,
  });

/**
 * PATCH /user/profile — dialect v2, 204 (§5.3, W5). §4.4 double-Option: an
 * absent flag retains the stored value, so callers send only what changed.
 */
export const update = (client: ApiClient, payload: UserUpdatePayload) =>
  client.request({
    url: '/user/profile',
    method: 'PATCH',
    dialect: 'v2',
    data: payload,
    responseSchema: noContentSchema,
  });

/** PUT /user/password — dialect v2, 204 (§5.3, W5). */
export const changePassword = (client: ApiClient, old_password: string, new_password: string) =>
  client.request({
    url: '/user/password',
    method: 'PUT',
    dialect: 'v2',
    data: { old_password, new_password },
    responseSchema: noContentSchema,
  });

/**
 * POST /user/subscription/reset-token — dialect v2 (§9.4, W5): rotates the
 * subscribe token and returns the freshly minted URL as `{subscribe_url}`.
 */
export const resetSecurity = (client: ApiClient) =>
  client
    .request({
      url: '/user/subscription/reset-token',
      method: 'POST',
      dialect: 'v2',
      responseSchema: resetSubscribeTokenSchema,
    })
    .then((body) => body.subscribe_url);

/**
 * POST /user/commission-transfers — dialect v2, 204 (docs/api-dialect.md
 * §5.3, W7). The `100*amount` cents conversion stays at this boundary;
 * callers pass decimal major units.
 */
export const transfer = (client: ApiClient, transferAmount: number | string | undefined) =>
  client.request({
    url: '/user/commission-transfers',
    method: 'POST',
    dialect: 'v2',
    data: { transfer_amount: decimalToCents(transferAmount ?? '') },
    responseSchema: noContentSchema,
  });

/** POST /user/subscription/new-period — dialect v2, 204 (§5.4, W5). */
export const newPeriod = (client: ApiClient) =>
  client.request({
    url: '/user/subscription/new-period',
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

export type RedeemGiftCardResult = output<typeof giftCardRedemptionSchema>;

/** POST /user/gift-card-redemptions — dialect v2 bare `{type, value}` (§9.4, W5). */
export const redeemGiftCard = (
  client: ApiClient,
  giftcard: string,
): Promise<RedeemGiftCardResult> =>
  client.request({
    url: '/user/gift-card-redemptions',
    method: 'POST',
    dialect: 'v2',
    data: { giftcard },
    responseSchema: giftCardRedemptionSchema,
  });

/** DELETE /user/telegram-binding — dialect v2, 204 (§5.3, W5). */
export const unbindTelegram = (client: ApiClient) =>
  client.request({
    url: '/user/telegram-binding',
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

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

/** GET /user/sessions — dialect v2 array with `session_id` (§9.4, W5). */
export const getActiveSession = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/sessions',
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(activeSessionSchema),
    ...config,
  });

/** DELETE /user/sessions/{session_id} — dialect v2, 204, idempotent (§9.4, W5). */
export const removeActiveSession = (client: ApiClient, session_id: string) =>
  client.request({
    url: `/user/sessions/${encodeURIComponent(session_id)}`,
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
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

/**
 * POST /user/invite-codes — dialect v2 (docs/api-dialect.md §5.6, W7): the
 * one deliberate 204-no-body create (§1). Invite codes are never
 * individually addressed afterwards, so callers refetch `GET /user/invite`
 * instead of consuming a created id.
 */
export const generateInvite = (client: ApiClient) =>
  client.request({
    url: '/user/invite-codes',
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/**
 * GET /user/invite — dialect v2 bare `{codes, stat}` with the §9.2 named
 * stat object (docs/api-dialect.md §5.6, W7). Commission values stay
 * integer cents.
 */
export const fetchInvite = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/invite',
    method: 'GET',
    dialect: 'v2',
    responseSchema: inviteFetchSchema,
    ...config,
  });

/**
 * GET /user/commissions — dialect v2 `{items, total}` page envelope on
 * `page`/`per_page` (docs/api-dialect.md §5.6/§8, W7; server default 10).
 * The raw requested page is sent unclamped — display clamping stays a
 * Tier-2 concern of the pagination control. Commission amounts stay cents
 * for the boundary `amount/100` display conversion.
 */
export const inviteDetails = async (
  client: ApiClient,
  page?: number,
  perPage?: number,
  config?: QueryRequestConfig,
): Promise<CommissionDetailPage> => {
  const result = await client.request({
    url: '/user/commissions',
    method: 'GET',
    dialect: 'v2',
    params: { page, per_page: perPage },
    responseSchema: pageSchema(commissionDetailSchema),
    ...config,
  });
  return { data: result.items, total: result.total };
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

/** GET /user/servers — dialect v2 bare array (§5.4, W6). */
export const fetchServers = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/servers',
    method: 'GET',
    dialect: 'v2',
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

/** GET /user/traffic-logs — dialect v2 bare array (§5.4, W6). */
export const getTrafficLog = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: '/user/traffic-logs',
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(trafficLogSchema),
    ...config,
  });
