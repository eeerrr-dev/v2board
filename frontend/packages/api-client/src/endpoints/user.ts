import type {
  CheckLoginResult,
  CommissionDetailPage,
  InternalApiOperationMap,
  Order,
  OrderCheckoutPayload,
  OrderCheckoutResult,
  PlanPeriod,
  StripePaymentIntentPayload,
  TicketCreatePayload,
  TicketReplyPayload,
  TicketWithdrawPayload,
  UserUpdatePayload,
  UserCoupon,
  UserPlan,
  UserStat,
} from '@v2board/types';
import type { ApiClient, ApiRequestConfig } from '../client';
import { bearerAuthorization } from '../dialect';
import { requestInternal } from '../internal-operation';
import { decimalToCents } from '../money';

type QueryRequestConfig = Pick<ApiRequestConfig, 'signal'>;
type GeneratedUserPlan = InternalApiOperationMap['userPlansGet']['response'];
type GeneratedUserOrder = InternalApiOperationMap['userOrdersGet']['response'];
type GeneratedCoupon = InternalApiOperationMap['userCouponsCheck']['response'];

const ORDER_PERIODS = new Set<string>([
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
  'deposit',
]);

function toUserPlan(plan: GeneratedUserPlan): UserPlan {
  const resetMethod = plan.reset_traffic_method;
  if (resetMethod !== null && ![0, 1, 2, 3, 4].includes(resetMethod)) {
    throw new TypeError(`Unsupported reset traffic method: ${resetMethod}`);
  }
  return {
    ...plan,
    reset_traffic_method: resetMethod as UserPlan['reset_traffic_method'],
  };
}

function toUserOrder(order: GeneratedUserOrder): Order {
  if (!ORDER_PERIODS.has(order.period)) {
    throw new TypeError(`Unsupported order period: ${order.period}`);
  }
  if (![1, 2, 3, 4, 9].includes(order.type)) {
    throw new TypeError(`Unsupported order type: ${order.type}`);
  }
  if (![0, 1, 2, 3, 4].includes(order.status)) {
    throw new TypeError(`Unsupported order status: ${order.status}`);
  }
  if (![0, 1, 2, 3].includes(order.commission_status)) {
    throw new TypeError(`Unsupported order commission status: ${order.commission_status}`);
  }
  const { plan, surplus_orders, try_out_plan_id, bounus, get_amount, ...base } = order;
  let normalizedPlan: Order['plan'];
  if (plan != null) {
    if ('capacity_limit' in plan) normalizedPlan = toUserPlan(plan);
    else {
      if (plan.id !== 0 || plan.name !== 'deposit') {
        throw new TypeError('Unsupported deposit order plan discriminator');
      }
      normalizedPlan = { id: 0, name: 'deposit' };
    }
  }
  return {
    ...base,
    period: order.period as Order['period'],
    type: order.type as Order['type'],
    status: order.status as Order['status'],
    commission_status: order.commission_status as Order['commission_status'],
    ...(normalizedPlan === undefined ? {} : { plan: normalizedPlan }),
    ...(surplus_orders == null ? {} : { surplus_orders: surplus_orders.map(toUserOrder) }),
    ...(try_out_plan_id == null ? {} : { try_out_plan_id }),
    ...(bounus == null ? {} : { bounus }),
    ...(get_amount == null ? {} : { get_amount }),
  };
}

function toUserCoupon(coupon: GeneratedCoupon): UserCoupon {
  if (coupon.type !== 1 && coupon.type !== 2) {
    throw new TypeError(`Unsupported coupon type: ${coupon.type}`);
  }
  return { ...coupon, type: coupon.type };
}

/** GET /user/profile — dialect v2 bare profile (docs/api-dialect.md §5.3, W5). */
export const info = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userProfileGet', {
    ...config,
  });

/** GET /user/stats — dialect v2 named counts (§9.1, W5). */
export const getStat = (client: ApiClient, config?: QueryRequestConfig): Promise<UserStat> =>
  requestInternal(client, 'userStatsGet', {
    ...config,
  });

// Session probe (GET /auth/session, dialect v2 — the checkLogin successor).
// A dead or absent bearer is data ({is_login: false}), never a 401.
export const checkLogin = async (
  client: ApiClient,
  config?: QueryRequestConfig,
): Promise<CheckLoginResult> => {
  const session = await requestInternal(client, 'authSessionGet', {
    ...config,
  });
  return {
    is_login: session.is_login,
    ...(session.is_admin == null ? {} : { is_admin: session.is_admin }),
    ...(session.is_staff == null ? {} : { is_staff: session.is_staff }),
    ...(session.admin_permissions == null ? {} : { admin_permissions: session.admin_permissions }),
  };
};

/** GET /user/subscription — dialect v2 bare body, explicit-null plan (§5.4, W5). */
export const getSubscribe = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userSubscriptionGet', {
    ...config,
  });

/**
 * PATCH /user/profile — dialect v2, 204 (§5.3, W5). §4.4 double-Option: an
 * absent flag retains the stored value, so callers send only what changed.
 */
export const update = (client: ApiClient, payload: UserUpdatePayload) =>
  requestInternal(client, 'userProfileUpdate', {
    data: payload,
  });

/** PUT /user/password — dialect v2, 204 (§5.3, W5). */
export const changePassword = (client: ApiClient, old_password: string, new_password: string) =>
  requestInternal(client, 'userPasswordUpdate', {
    data: { old_password, new_password },
  });

/**
 * POST /user/subscription/reset-token — dialect v2 (§9.4, W5): rotates the
 * subscribe token and returns the freshly minted URL as `{subscribe_url}`.
 */
export const resetSecurity = (client: ApiClient) =>
  requestInternal(client, 'userSubscriptionResetToken', {}).then((body) => body.subscribe_url);

/**
 * POST /user/commission-transfers — dialect v2, 204 (docs/api-dialect.md
 * §5.3, W7). The `100*amount` cents conversion stays at this boundary;
 * callers pass decimal major units.
 */
export const transfer = (client: ApiClient, transferAmount: number | string | undefined) =>
  requestInternal(client, 'userCommissionTransfersCreate', {
    data: { transfer_amount: decimalToCents(transferAmount ?? '') },
  });

/** POST /user/subscription/new-period — dialect v2, 204 (§5.4, W5). */
export const newPeriod = (client: ApiClient) =>
  requestInternal(client, 'userSubscriptionNewPeriod', {});

export type RedeemGiftCardResult =
  InternalApiOperationMap['userGiftCardRedemptionsCreate']['response'];

/** POST /user/gift-card-redemptions — dialect v2 bare `{type, value}` (§9.4, W5). */
export const redeemGiftCard = (
  client: ApiClient,
  giftcard: string,
): Promise<RedeemGiftCardResult> =>
  requestInternal(client, 'userGiftCardRedemptionsCreate', {
    data: { giftcard },
  });

/** DELETE /user/telegram-binding — dialect v2, 204 (§5.3, W5). */
export const unbindTelegram = (client: ApiClient) =>
  requestInternal(client, 'userTelegramBindingDelete', {});

// Explicit sign-out: best-effort server-side revocation of the current opaque
// session (DELETE /auth/session, 204 — a Rust-only endpoint; the legacy API
// had no logout). The caller tears local auth down synchronously right after
// firing this, and the client's request interceptor reads the auth store on a
// microtask — after that teardown — so the raw auth_data must be captured up
// front and passed here; the endpoint puts the Bearer scheme on the wire
// (§4.2). The backend treats a dead or absent bearer as a successful no-op.
export const logout = (client: ApiClient, capturedAuthData?: string | null) => {
  const authorization = bearerAuthorization(capturedAuthData);
  return requestInternal(client, 'authSessionDelete', {
    ...(authorization ? { headers: { authorization } } : {}),
  });
};

/** GET /user/sessions — dialect v2 array with `session_id` (§9.4, W5). */
export const getActiveSession = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userSessionsList', {
    ...config,
  });

/** DELETE /user/sessions/{session_id} — dialect v2, 204, idempotent (§9.4, W5). */
export const removeActiveSession = (client: ApiClient, session_id: string) =>
  requestInternal(client, 'userSessionsDelete', {
    path: { session_id },
  });

/** GET /user/plans — dialect v2 bare array (docs/api-dialect.md §5.5, W4). */
export const fetchPlans = async (client: ApiClient, config?: QueryRequestConfig) =>
  (
    await requestInternal(client, 'userPlansList', {
      ...config,
    })
  ).map(toUserPlan);

/** GET /user/plans/{id} — dialect v2 bare plan; a miss is 404 plan_not_found (§5.5, W4). */
export const fetchPlan = async (
  client: ApiClient,
  id: number | string,
  config?: QueryRequestConfig,
) =>
  toUserPlan(
    await requestInternal(client, 'userPlansGet', {
      path: { id: Number(id) },
      ...config,
    }),
  );

/** GET /user/orders?status= — dialect v2 bare array (§5.5, W4). */
export const fetchOrders = async (
  client: ApiClient,
  status?: number,
  config?: QueryRequestConfig,
) =>
  (
    await requestInternal(client, 'userOrdersList', {
      query: status === undefined ? {} : { status },
      ...config,
    })
  ).map(toUserOrder);

/** GET /user/orders/{trade_no} — dialect v2 bare order (§5.5, W4). */
export const orderDetail = async (
  client: ApiClient,
  trade_no: string,
  config?: QueryRequestConfig,
) =>
  toUserOrder(
    await requestInternal(client, 'userOrdersGet', {
      path: { trade_no },
      ...config,
    }),
  );

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
  const data: InternalApiOperationMap['userOrdersCreate']['request'] =
    payload.kind === 'deposit'
      ? { kind: 'deposit', deposit_amount: decimalToCents(payload.deposit_amount) }
      : payload;
  const created = await requestInternal(client, 'userOrdersCreate', {
    data,
  });
  return created.trade_no;
};

/** POST /user/orders/{trade_no}/checkout — dialect v2, the §9.3 result union (W4). */
export const checkoutOrder = (
  client: ApiClient,
  payload: OrderCheckoutPayload,
): Promise<OrderCheckoutResult> =>
  requestInternal(client, 'userOrdersCheckout', {
    path: { trade_no: payload.trade_no },
    data: { method_id: payload.method_id },
  });

/**
 * GET /user/orders/{trade_no}/status — dialect v2 `{status}` body (§9.4, W4);
 * resolves to the bare status number the 3s poll consumes.
 */
export const checkOrder = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  requestInternal(client, 'userOrdersStatus', {
    path: { trade_no },
    ...config,
  }).then((body) => body.status);

/** POST /user/orders/{trade_no}/cancel — dialect v2, trade_no in the path, 204 (§5.5, W4). */
export const cancelOrder = (client: ApiClient, trade_no: string) =>
  requestInternal(client, 'userOrdersCancel', {
    path: { trade_no },
  });

/** GET /user/payment-methods — dialect v2 bare array, numeric percent (§5.5, W4). */
export const getPaymentMethod = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userPaymentMethodsList', {
    ...config,
  });

/**
 * POST /user/invite-codes — dialect v2 (docs/api-dialect.md §5.6, W7): the
 * one deliberate 204-no-body create (§1). Invite codes are never
 * individually addressed afterwards, so callers refetch `GET /user/invite`
 * instead of consuming a created id.
 */
export const generateInvite = (client: ApiClient) =>
  requestInternal(client, 'userInviteCodesCreate', {});

/**
 * GET /user/invite — dialect v2 bare `{codes, stat}` with the §9.2 named
 * stat object (docs/api-dialect.md §5.6, W7). Commission values stay
 * integer cents.
 */
export const fetchInvite = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userInviteGet', {
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
  const result = await requestInternal(client, 'userCommissionsList', {
    query: { page, per_page: perPage },
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
  const page = await requestInternal(client, 'userNoticesList', {
    ...config,
  });
  return page.items;
};

/** GET /user/tickets — dialect v2 bare array (docs/api-dialect.md §5.7, W8). */
export const fetchTickets = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userTicketsList', {
    ...config,
  });

/** GET /user/tickets/{id} — dialect v2 bare detail with the `message[]` thread (§5.7, W8). */
export const ticketDetail = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  requestInternal(client, 'userTicketsGet', {
    path: { id: Number(id) },
    ...config,
  });

/** POST /user/tickets — dialect v2 JSON `{subject, level, message}` → 201 `{id}` (§5.7, W8). */
export const saveTicket = (client: ApiClient, payload: TicketCreatePayload) =>
  requestInternal(client, 'userTicketsCreate', { data: requiredTicketCreate(payload) });

function requiredTicketCreate(
  payload: TicketCreatePayload,
): InternalApiOperationMap['userTicketsCreate']['request'] {
  const { subject, level, message } = payload;
  if (subject === undefined || level === undefined || message === undefined) {
    throw new TypeError('Ticket subject, level, and message are required');
  }
  return { subject, level, message };
}

/** POST /user/tickets/{id}/replies — dialect v2, 204; the `id` moves to the path (§5.7, W8). */
export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) => {
  const { id, message } = payload;
  if (message === undefined) throw new TypeError('Ticket reply message is required');
  return requestInternal(client, 'userTicketsRepliesCreate', {
    path: { id: Number(id) },
    data: { message },
  });
};

/** POST /user/tickets/{id}/close — dialect v2, 204, no body (§5.7, W8). */
export const closeTicket = (client: ApiClient, id: number) =>
  requestInternal(client, 'userTicketsClose', {
    path: { id },
  });

/** POST /user/withdrawal-tickets — dialect v2 JSON → 201 `{id}` (§5.7, W8). */
export const withdrawTicket = (client: ApiClient, payload: TicketWithdrawPayload) =>
  requestInternal(client, 'userWithdrawalTicketsCreate', {
    data: requiredWithdrawalTicket(payload),
  });

function requiredWithdrawalTicket(
  payload: TicketWithdrawPayload,
): InternalApiOperationMap['userWithdrawalTicketsCreate']['request'] {
  const { withdraw_method, withdraw_account } = payload;
  if (withdraw_method === undefined || withdraw_account === undefined) {
    throw new TypeError('Withdrawal method and account are required');
  }
  return { withdraw_method, withdraw_account };
}

/** GET /user/servers — dialect v2 bare array (§5.4, W6). */
export const fetchServers = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userServersList', {
    ...config,
  });

/** POST /user/coupons/check — dialect v2 JSON `{code, plan_id}` → bare coupon (§5.5, W4). */
export const checkCoupon = async (client: ApiClient, code: string, plan_id: number | string) =>
  toUserCoupon(
    await requestInternal(client, 'userCouponsCheck', {
      data: { code, plan_id: Number(plan_id) },
    }),
  );

/** GET /user/telegram-bot — dialect v2 bare `{username}` (§5.3, W3). */
export const getTelegramBotInfo = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userTelegramBotGet', {
    ...config,
  });

/** GET /user/config — dialect v2 bare body (§5.3, W3). */
export const commConfig = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userConfigGet', {
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
  requestInternal(client, 'userOrdersStripeIntent', {
    path: { trade_no: payload.trade_no },
    data: { method_id: payload.method_id },
    ...config,
  });

/** GET /user/knowledge — dialect v2 bare `{category: [...]}` record (§5.8, W3). */
export const fetchKnowledge = (
  client: ApiClient,
  language: string,
  keyword?: string,
  config?: QueryRequestConfig,
) =>
  requestInternal(client, 'userKnowledgeList', {
    query: { language, keyword },
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
) => {
  // Language remains part of the query key/caller API so a locale switch
  // refetches the non-idempotent body. The modern detail route itself selects
  // the stored article by id and has no language query parameter.
  void language;
  return requestInternal(client, 'userKnowledgeGet', {
    path: { id: Number(id) },
    ...config,
  });
};

/** GET /user/traffic-logs — dialect v2 bare array (§5.4, W6). */
export const getTrafficLog = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'userTrafficLogsList', {
    ...config,
  });
