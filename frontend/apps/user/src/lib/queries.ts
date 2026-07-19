import { user } from '@v2board/api-client';
import {
  keepPreviousData,
  queryOptions,
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import type { UseQueryResult } from '@tanstack/react-query';
import type { StripePaymentIntent, SubscribeInfo, UserInfo } from '@v2board/types';
import dayjs from 'dayjs';
import { formatBytes } from '@v2board/config/format';
import { apiClient } from './api';

export type SaveOrderInput = Parameters<typeof user.saveOrder>[1];
export type CheckoutOrderPayload = Parameters<typeof user.checkoutOrder>[1];

interface QueryFreshnessOptions {
  enabled?: boolean;
  refetchInterval?: number | false;
  refetchOnMount?: boolean | 'always';
}

declare global {
  interface Window {
    Tawk_API?: { visitor?: { name?: string; email?: string } };
    $crisp?: { push: (command: unknown[]) => void };
  }
}

// The original getUserInfo / getSubscribe sagas report the user to the Tawk and
// Crisp live-chat widgets right after each successful fetch. React Query v5
// removed the per-observer useQuery onSuccess; its canonical replacement is the
// QueryClient-level QueryCache onSuccess, which fires once per successful fetch
// keyed by query. main.tsx wires these reporters there (on userKeys.info /
// userKeys.subscribe), so the queryFns below stay pure while preserving the
// saga's "report after each successful 200" cadence, including refetches. The
// Crisp/Tawk payloads are external-integration contracts — keep their shape.
export function reportUserInfoToChat(info: UserInfo) {
  if (window.Tawk_API) {
    window.Tawk_API.visitor = { name: info.email, email: info.email };
  }
  if (window.$crisp) {
    window.$crisp.push(['set', 'user:email', info.email]);
    window.$crisp.push(['set', 'session:data', [[['Balance', info.balance / 100]]]]);
  }
}

export function reportSubscribeToChat(data: SubscribeInfo) {
  if (!window.$crisp) return;
  // The external chat contract formats `expired_at` in local time; a null expiry
  // becomes the epoch date (there is intentionally no '-' fallback here). The
  // wire value is RFC 3339 (§4.5); the Crisp 'YYYY-MM-DD' output stays frozen.
  const expireTime = dayjs(data.expired_at ?? 0).format('YYYY-MM-DD');
  window.$crisp.push([
    'set',
    'session:data',
    [
      [
        ['Plan', data.plan?.name || '-'],
        ['ExpireTime', expireTime],
        ['UsedTraffic', formatBytes(data.u + data.d)],
        ['AllTraffic', formatBytes(data.transfer_enable)],
      ],
    ],
  ]);
}

// Shared prefix for every order query (list + detail + status). Kept as a single
// source so a scope-wide invalidation (useCancelOrderMutation) routes through the
// factory instead of a raw literal that a future rename would silently miss.
const ordersScope = ['user', 'orders'] as const;

export const userKeys = {
  checkLogin: ['user', 'checkLogin'] as const,
  info: ['user', 'info'] as const,
  stat: ['user', 'stat'] as const,
  subscribe: ['user', 'subscribe'] as const,
  ordersScope,
  orders: (status?: number) => [...ordersScope, status ?? 'all'] as const,
  // The id params are honestly `| undefined`: an unset route id lands a literal
  // undefined in the key, inert because the matching queryOptions factory swaps
  // its queryFn for skipToken (never a `?? ''` sentinel, which would be a
  // distinct, request-reaching key).
  orderDetail: (tradeNo: string | undefined) => [...ordersScope, 'detail', tradeNo] as const,
  orderStatus: (tradeNo: string | undefined) => [...ordersScope, 'status', tradeNo] as const,
  plans: ['user', 'plans'] as const,
  plan: (id: number | string | undefined) => ['user', 'plan', id] as const,
  payments: ['user', 'payments'] as const,
  notices: ['user', 'notices'] as const,
  tickets: ['user', 'tickets'] as const,
  ticketDetail: (id: number | string | undefined) => ['user', 'ticket', id] as const,
  invite: ['user', 'invite'] as const,
  inviteDetails: (current?: number, size?: number) =>
    ['user', 'invite', 'details', current ?? '', size ?? ''] as const,
  knowledge: (lang: string, kw?: string) => ['user', 'knowledge', lang, kw ?? ''] as const,
  knowledgeDetail: (id: number | string | undefined, lang: string) =>
    ['user', 'knowledge', 'detail', id, lang] as const,
  trafficLog: ['user', 'trafficLog'] as const,
  commConfig: ['user', 'comm'] as const,
  stripePaymentIntent: (tradeNo: string | undefined, methodId: number | undefined) =>
    ['user', 'stripePaymentIntent', tradeNo, methodId] as const,
  servers: ['user', 'servers'] as const,
  telegramBot: ['user', 'telegram', 'bot'] as const,
  sessions: ['user', 'sessions'] as const,
};

export function fetchUserInfo(signal?: AbortSignal) {
  return user.info(apiClient, { signal });
}

function fetchSubscribe(signal?: AbortSignal) {
  return user.getSubscribe(apiClient, { signal });
}

export const userQueryOptions = {
  checkLogin: () =>
    queryOptions({
      queryKey: userKeys.checkLogin,
      queryFn: ({ signal }) => user.checkLogin(apiClient, { signal }),
      // Match the former one-shot effect: do not retry an auth probe, and do
      // not treat a prior navigation's result as fresh on the next /login entry.
      retry: false,
      staleTime: 0,
    }),
  info: () =>
    queryOptions({
      queryKey: userKeys.info,
      queryFn: ({ signal }) => fetchUserInfo(signal),
    }),
  stat: () =>
    queryOptions({
      queryKey: userKeys.stat,
      queryFn: ({ signal }) => user.getStat(apiClient, { signal }),
    }),
  subscribe: () =>
    queryOptions({
      queryKey: userKeys.subscribe,
      queryFn: ({ signal }) => fetchSubscribe(signal),
    }),
  orders: (status?: number) =>
    queryOptions({
      queryKey: userKeys.orders(status),
      queryFn: ({ signal }) => user.fetchOrders(apiClient, status, { signal }),
    }),
  orderDetail: (tradeNo: string | undefined) =>
    queryOptions({
      queryKey: userKeys.orderDetail(tradeNo),
      queryFn: !tradeNo
        ? skipToken
        : ({ signal }) => user.orderDetail(apiClient, tradeNo, { signal }),
    }),
  orderStatus: (tradeNo: string | undefined) =>
    queryOptions({
      queryKey: userKeys.orderStatus(tradeNo),
      queryFn: !tradeNo
        ? skipToken
        : ({ signal }) => user.checkOrder(apiClient, tradeNo, { signal }),
    }),
  plans: () =>
    queryOptions({
      queryKey: userKeys.plans,
      queryFn: ({ signal }) => user.fetchPlans(apiClient, { signal }),
    }),
  plan: (id: number | string | undefined) =>
    queryOptions({
      queryKey: userKeys.plan(id),
      queryFn: !id ? skipToken : ({ signal }) => user.fetchPlan(apiClient, id, { signal }),
    }),
  payments: () =>
    queryOptions({
      queryKey: userKeys.payments,
      queryFn: ({ signal }) => user.getPaymentMethod(apiClient, { signal }),
    }),
  notices: () =>
    queryOptions({
      queryKey: userKeys.notices,
      queryFn: ({ signal }) => user.fetchNotices(apiClient, { signal }),
    }),
  tickets: () =>
    queryOptions({
      queryKey: userKeys.tickets,
      queryFn: ({ signal }) => user.fetchTickets(apiClient, { signal }),
    }),
  ticketDetail: (id: number | string | undefined) =>
    queryOptions({
      queryKey: userKeys.ticketDetail(id),
      queryFn: !id ? skipToken : ({ signal }) => user.ticketDetail(apiClient, id, { signal }),
    }),
  invite: () =>
    queryOptions({
      queryKey: userKeys.invite,
      queryFn: ({ signal }) => user.fetchInvite(apiClient, { signal }),
    }),
  inviteDetails: (current?: number, pageSize?: number) =>
    queryOptions({
      queryKey: userKeys.inviteDetails(current, pageSize),
      queryFn: ({ signal }) => user.inviteDetails(apiClient, current, pageSize, { signal }),
    }),
  knowledge: (language: string, keyword?: string) =>
    queryOptions({
      queryKey: userKeys.knowledge(language, keyword),
      // Pass the query's AbortSignal so a superseded debounced search GET is
      // cancelled instead of running to completion.
      queryFn: ({ signal }) => user.fetchKnowledge(apiClient, language, keyword, { signal }),
    }),
  knowledgeDetail: (id: number | string | undefined, language: string) =>
    queryOptions({
      queryKey: userKeys.knowledgeDetail(id, language),
      queryFn:
        id === undefined
          ? skipToken
          : ({ signal }) => user.knowledgeDetail(apiClient, id, language, { signal }),
    }),
  trafficLog: () =>
    queryOptions({
      queryKey: userKeys.trafficLog,
      queryFn: ({ signal }) => user.getTrafficLog(apiClient, { signal }),
    }),
  commConfig: () =>
    queryOptions({
      queryKey: userKeys.commConfig,
      queryFn: ({ signal }) => user.commConfig(apiClient, { signal }),
    }),
  stripePaymentIntent: (tradeNo: string | undefined, methodId: number | undefined) =>
    queryOptions({
      queryKey: userKeys.stripePaymentIntent(tradeNo, methodId),
      queryFn:
        !tradeNo || !methodId
          ? skipToken
          : ({ signal }) =>
              user.prepareStripePaymentIntent(
                apiClient,
                { trade_no: tradeNo, method_id: methodId },
                { signal },
              ),
    }),
  servers: () =>
    queryOptions({
      queryKey: userKeys.servers,
      queryFn: ({ signal }) => user.fetchServers(apiClient, { signal }),
    }),
  telegramBot: () =>
    queryOptions({
      queryKey: userKeys.telegramBot,
      queryFn: ({ signal }) => user.getTelegramBotInfo(apiClient, { signal }),
    }),
  sessions: () =>
    queryOptions({
      queryKey: userKeys.sessions,
      queryFn: ({ signal }) => user.getActiveSession(apiClient, { signal }),
    }),
};

export const useUserInfo = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.info(),
    ...options,
  });

export const useUserStat = () => useQuery(userQueryOptions.stat());

export const useSubscribe = (options?: QueryFreshnessOptions & { enabled?: boolean }) =>
  useQuery({
    ...userQueryOptions.subscribe(),
    ...options,
  });

export const useOrders = (status?: number) => useQuery(userQueryOptions.orders(status));

export const useOrder = (tradeNo: string | undefined) =>
  useQuery(userQueryOptions.orderDetail(tradeNo));

export const useOrderStatus = (tradeNo: string | undefined, options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.orderStatus(tradeNo),
    // The poll's stop condition is intrinsic to /user/order/check, not a caller
    // choice: keep refetching every 3s while the order is still pending (status
    // 0 or not yet fetched) and self-stop the moment it leaves pending or the
    // check errors. The caller only decides whether to poll at all, via enabled.
    refetchInterval: (query) =>
      query.state.status === 'error' || (query.state.data ?? 0) !== 0 ? false : 3000,
    ...options,
  });

export const usePlans = () => useQuery(userQueryOptions.plans());

export const usePlan = (id: number | string | undefined) => useQuery(userQueryOptions.plan(id));

export const usePaymentMethods = (options?: QueryFreshnessOptions & { enabled?: boolean }) =>
  useQuery({
    ...userQueryOptions.payments(),
    // Payment gateways are operator configuration that rarely changes, so avoid
    // refetching them on every checkout mount. (Plans are deliberately NOT given
    // a stale window: their capacity_limit/sold-out state is a live contract.)
    staleTime: 5 * 60_000,
    ...options,
  });

export const useNotices = () => useQuery(userQueryOptions.notices());

export const useTickets = () => useQuery(userQueryOptions.tickets());

export const useTicket = (id: number | string | undefined, options?: QueryFreshnessOptions) => {
  const { refetchInterval, ...rest } = options ?? {};
  return useQuery({
    ...userQueryOptions.ticketDetail(id),
    // Like the order-status poll, the stop condition is intrinsic: a closed
    // ticket (status 1) or a missing/errored one must not keep re-requesting
    // for as long as the page stays mounted. Callers only pick the cadence.
    refetchInterval:
      typeof refetchInterval === 'number'
        ? (query) =>
            query.state.status === 'error' || query.state.data?.status === 1
              ? false
              : refetchInterval
        : refetchInterval,
    ...rest,
  });
};

export const useInvite = () => useQuery(userQueryOptions.invite());

export const useInviteDetails = (current?: number, pageSize?: number) =>
  useQuery({
    ...userQueryOptions.inviteDetails(current, pageSize),
    // Keep the current page visible while the next page request is in flight.
    placeholderData: keepPreviousData,
  });

export const useKnowledge = (language: string, keyword?: string) =>
  useQuery({
    ...userQueryOptions.knowledge(language, keyword),
    // Keep the current result list visible while debounced searches resolve.
    placeholderData: keepPreviousData,
  });

export const useKnowledgeDetail = (id: number | string | undefined, language: string) =>
  useQuery(userQueryOptions.knowledgeDetail(id, language));

export const useTrafficLog = () => useQuery(userQueryOptions.trafficLog());

export const useCommConfig = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.commConfig(),
    ...options,
  });

export function useStripePaymentIntent(
  tradeNo: string | undefined,
  methodId: number | undefined,
  options?: { enabled?: boolean },
): UseQueryResult<StripePaymentIntent> {
  return useQuery({
    ...userQueryOptions.stripePaymentIntent(tradeNo, methodId),
    enabled: options?.enabled !== false,
    // A PaymentIntent is bound to the currently selected gateway in the order row.
    // Do not resurrect method A's cached client secret after A -> B -> A: B's
    // preparation has already canceled A. The server endpoint is idempotent and
    // safely reuses the still-current intent on a real remount.
    staleTime: 0,
    gcTime: 0,
    refetchOnMount: 'always',
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
}

export const useServers = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.servers(),
    ...options,
  });

export const useTelegramBotInfo = (enabled: boolean) =>
  useQuery({
    ...userQueryOptions.telegramBot(),
    enabled,
    staleTime: 0,
  });

export const useActiveSessions = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.sessions(),
    ...options,
  });

export function useChangePasswordMutation() {
  return useMutation({
    mutationFn: ({ oldPassword, newPassword }: { oldPassword: string; newPassword: string }) =>
      user.changePassword(apiClient, oldPassword, newPassword),
  });
}

export function useUpdateProfileMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.update>[1]) => user.update(apiClient, payload),
    // The payload carries only the three preference booleans, so the cached
    // user record flips optimistically — the switches answer on click instead
    // of after the round trip. An error restores the snapshot (the global
    // mutation toast reports the failure); success still refetches the
    // authoritative record here so no call site wires its own refetch.
    onMutate: async (payload) => {
      await queryClient.cancelQueries({ queryKey: userKeys.info });
      const snapshot = queryClient.getQueryData<UserInfo>(userKeys.info);
      if (snapshot) queryClient.setQueryData(userKeys.info, { ...snapshot, ...payload });
      return { snapshot };
    },
    onError: (_error, _payload, context) => {
      if (context?.snapshot) queryClient.setQueryData(userKeys.info, context.snapshot);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useResetSubscribeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.resetSecurity(apiClient),
    // resetSecurity rotates the UUID embedded in the subscription URL. Mark both
    // projections stale at the mutation boundary so every screen observes the new
    // credential instead of retaining the previous token until a later navigation.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
      void queryClient.invalidateQueries({ queryKey: userKeys.subscribe });
    },
  });
}

export function useTransferMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (amount: number | string | undefined) => user.transfer(apiClient, amount),
    // Transfer moves commission into the balance shown by the user record.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useRedeemGiftCardMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => user.redeemGiftCard(apiClient, code),
    // A redeemed gift card credits the balance on the user record.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useCheckCouponMutation() {
  return useMutation({
    mutationFn: ({ code, planId }: { code: string; planId: number | string }) =>
      user.checkCoupon(apiClient, code, planId),
  });
}

export function useSaveOrderMutation() {
  return useMutation({
    mutationFn: (payload: SaveOrderInput) => user.saveOrder(apiClient, payload),
  });
}

export function useCheckoutOrderMutation() {
  return useMutation({
    mutationFn: (payload: CheckoutOrderPayload) => user.checkoutOrder(apiClient, payload),
  });
}

export function useGenerateInviteMutation() {
  return useMutation({
    mutationFn: () => user.generateInvite(apiClient),
  });
}

export function useWithdrawCommissionMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.withdrawTicket>[1]) =>
      user.withdrawTicket(apiClient, payload),
  });
}

export function useSaveTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.saveTicket>[1]) =>
      user.saveTicket(apiClient, payload),
    // Creating a ticket adds it to the list; invalidate here so the mutation
    // owns the refresh instead of each call site wiring its own.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.tickets });
    },
  });
}

export function useReplyTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.replyTicket>[1]) =>
      user.replyTicket(apiClient, payload),
    // Replying appends to the detail thread; invalidate that ticket's detail
    // query from the mutation so the chat refreshes right away instead of
    // waiting on the detail page's 5s poll.
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: userKeys.ticketDetail(variables.id) });
    },
  });
}

export function useCloseTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => user.closeTicket(apiClient, id),
    // Closing a ticket changes its status in the list; invalidate from the
    // mutation so the refresh is centralized like the other list mutations.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.tickets });
    },
  });
}

export function useCancelOrderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tradeNo: string) => user.cancelOrder(apiClient, tradeNo),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.ordersScope });
    },
  });
}

export function useNewPeriodMutation() {
  return useMutation({
    mutationFn: () => user.newPeriod(apiClient),
  });
}

export function useUnbindTelegramMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.unbindTelegram(apiClient),
    // Unbinding clears telegram_id on the user record. The optional subscribe
    // cache is unrelated to that state and does not need a parity-only refetch.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useRemoveSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (sessionId: string) => user.removeActiveSession(apiClient, sessionId),
    // Revoking drops one entry from the session list; invalidate so the list
    // refetches instead of the call site wiring its own refresh.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.sessions });
    },
  });
}
